import os
import time
import pickle
import select
import subprocess

from pueue.helper.paths import createDir, createLogDir
from pueue.helper.socket import getSocketName, getDaemonSocket


class Daemon():
    def __init__(self):
        # Create config dir, if not existing
        self.queueFolder = createDir()
        self.logDir = createLogDir()

        self.readQueue()
        self.socket = getDaemonSocket()
        if len(self.queue) != 0:
            self.nextKey = max(self.queue.keys()) + 1
            self.readLog(False)
            self.paused = True
        else:
            self.nextKey = 0
            self.paused = False
            self.readLog(True)
        self.currentKey = None
        self.current = None

        # Daemon states
        self.clientAddress = None
        self.clientSocket = None
        self.process = None
        self.read_list = [self.socket]

    def respondClient(self, answer):
        response = pickle.dumps(answer, -1)
        self.clientSocket.send(response)
        self.read_list.remove(self.clientSocket)
        self.clientSocket.close()

    def main(self):
        while True:
            readable, writable, errored = select.select(self.read_list, [], [], 1)
            for s in readable:
                if s is self.socket:
                    try:
                        self.clientSocket, self.clientAddress = self.socket.accept()
                        self.read_list.append(self.clientSocket)
                    except:
                        print('Daemon rejected client')
                else:
                    try:
                        instruction = self.clientSocket.recv(8192)
                    except EOFError:
                        print('Client died while sending message, dropping received data.')
                        instruction = -1

                    if instruction is not -1:
                        try:
                            command = pickle.loads(instruction)
                        except EOFError:
                            print('Received message is incomplete, dropping received data.')
                            self.read_list.remove(self.clientSocket)
                            self.clientSocket.close()

                            command = {}
                            command['mode'] = ''

                        if command['mode'] == 'add':

                            # Add command to queue and save it
                            self.queue[self.nextKey] = command
                            self.nextKey += 1
                            self.writeQueue()
                            self.respondClient('Command added')

                        elif command['mode'] == 'remove':
                            key = command['key']
                            if key not in self.queue:
                                # Send error answer to client in case there exists no such key
                                answer = 'No command with key #' + str(key)
                            else:
                                # Delete command from queue, save the queue and send response to client
                                if not self.paused and key == self.currentKey:
                                    answer = "Can't remove currently running process, please stop the process before removing it."
                                else:
                                    del self.queue[key]
                                    self.writeQueue()
                                    answer = 'Command #'+str(key)+' removed'
                            self.respondClient(answer)

                        elif command['mode'] == 'show':
                            answer = {}
                            data = []
                            # Process status
                            if (self.process is not None):
                                self.process.poll()
                                if self.process.returncode is None:
                                    answer['process'] = 'running'
                            elif self.currentKey in self.log.keys():
                                answer['process'] = 'finished'.format(self.log[self.currentKey]['returncode'])
                            else:
                                answer['process'] = 'no process'

                            if self.paused:
                                answer['status'] = 'paused'
                            else:
                                answer['status'] = 'running'
                            if self.currentKey in self.log.keys():
                                answer['current'] = self.log[self.currentKey]['returncode']
                            else:
                                answer['current'] = 'No exitcode'

                            # Queue status
                            if command['index'] == 'all':
                                if len(self.queue) > 0:
                                    data = self.queue
                                else:
                                    data = 'Queue is empty'
                            answer['data'] = data

                            # Respond client
                            self.respondClient(answer)

                        elif command['mode'] == 'reset':
                            # Reset  queue
                            self.queue = {}
                            self.writeQueue()
                            # Terminate current process
                            if self.process is not None:
                                self.process.terminate()
                            # Rotate and reset Log
                            self.readLog(True)
                            self.writeLog()
                            self.currentKey = None
                            self.nextKey = 0
                            answer = 'Reseting current queue'
                            self.respondClient(answer)

                        elif command['mode'] == 'START':
                            if self.paused:
                                self.paused = False
                                answer = 'Daemon started'
                            else:
                                answer = 'Daemon alrady started'
                            self.respondClient(answer)

                        elif command['mode'] == 'PAUSE':
                            if not self.paused:
                                self.paused = True
                                answer = 'Daemon paused'
                            else:
                                answer = 'Daemon already paused'
                            self.respondClient(answer)

                        elif command['mode'] == 'STOP':
                            if (self.process is not None):
                                self.process.poll()
                                if self.process.returncode is None:
                                    self.paused = True
                                    self.process.terminate()
                                    answer = 'Terminating current process and pausing'
                                else:
                                    answer = "No process running, pausing daemon"
                            else:
                                answer = "No process running, pausing daemon"
                            self.respondClient(answer)

                        elif command['mode'] == 'KILL':
                            if (self.process is not None):
                                self.process.poll()
                                if self.process.returncode is None:
                                    self.paused = True
                                    self.process.kill()
                                    answer = 'Sent kill to process and paused daemon'
                                else:
                                    answer = "Process just terminated on it's own"
                            else:
                                answer = 'No process running, pausing daemon'
                            self.respondClient(answer)

                        elif command['mode'] == 'EXIT':
                            self.respondClient('Pueue daemon shutting down')
                            break

            if self.process is not None:
                self.process.poll()
                if self.process.returncode is not None:
                    output, error_output = self.process.communicate()
                    self.log[min(self.queue.keys())] = self.queue[min(self.queue.keys())]
                    self.log[min(self.queue.keys())]['stderr'] = error_output
                    self.log[min(self.queue.keys())]['stdout'] = output
                    self.log[min(self.queue.keys())]['returncode'] = self.process.returncode
                    self.queue.pop(min(self.queue.keys()), None)
                    self.writeQueue()
                    self.writeLog()
                    self.process = None

            elif not self.paused:
                if (len(self.queue) > 0):
                    self.currentKey = min(self.queue.keys())
                    next_item = self.queue[self.currentKey]
                    self.process = subprocess.Popen(
                            next_item['command'],
                            shell=True,
                            stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE,
                            universal_newlines=True,
                            cwd=next_item['path'])

        self.socket.close()
        os.remove(getSocketName())

    def readQueue(self):
        queuePath = self.queueFolder+'/queue'
        if os.path.exists(queuePath):
            queueFile = open(queuePath, 'rb')
            try:
                self.queue = pickle.load(queueFile)
            except:
                print("Queue file corrupted, deleting old queue")
                os.remove(queuePath)
                self.queue = {}
            queueFile.close()
        else:
            self.queue = {}

    def writeQueue(self):
        home = os.path.expanduser('~')
        queuePath = home+'/.pueue/queue'
        queueFile = open(queuePath, 'wb+')
        try:
            pickle.dump(self.queue, queueFile, -1)
        except:
            print("Error while writing to queue file. Wrong file permissions?")
        queueFile.close()

    def readLog(self, rotate=False):
        logPath = self.queueFolder + '/queue.picklelog'
        if os.path.exists(logPath):
            logFile = open(logPath, 'rb')
            try:
                self.log = pickle.load(logFile)
            except:
                print("Log file corrupted, deleting old log")
                os.remove(logPath)
                self.log = {}
            logFile.close()
        else:
            self.log = {}

        if rotate:
            self.writeLog(True)
            os.remove(logPath)
            self.log = {}
            self.writeLog()

    def writeLog(self, rotate=False):

        if rotate:
            timestamp = time.strftime("%Y%m%d-%H%M")
            logPath = self.logDir + '/queue-' + timestamp + '.log'
        else:
            logPath = self.logDir + '/queue.log'

        picklelogPath = self.queueFolder + '/queue.picklelog'
        picklelogFile = open(picklelogPath, 'wb+')
        if os.path.exists(logPath):
            os.remove(logPath)
        logFile = open(logPath, 'w')
        logFile.write('Pueue log for executed Commands: \n \n \n')
        for key in self.log:
            try:
                logFile.write('Command #{} exited with returncode {}: \n    '.format(key, self.log[key]['returncode']))
                logFile.write(self.log[key]['command'] + '\n')
                logFile.write('Path: \n    ')
                logFile.write(self.log[key]['path'] + '\n')
                if self.log[key]['stderr']:
                    logFile.write('Stderr output: \n')
                    logFile.write(self.log[key]['stderr'] + '\n')
                logFile.write('Stdout output: \n')
                logFile.write(self.log[key]['stdout'] + '\n')
                logFile.write('\n \n')
            except:
                print("Error while writing to log file. Wrong file permissions?")
        try:
            pickle.dump(self.log, picklelogFile, -1)
        except:
            print("Error while writing to picklelog file. Wrong file permissions?")
        logFile.close()
