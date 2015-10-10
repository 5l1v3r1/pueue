from setuptools import setup, find_packages

setup(
    name='pueue',
    author='Arne Beer',
    author_email='arne@twobeer.de',
    version='0.3.0',
    description='Pueue is a fancy queue for bash commands',
    keywords='bash queue command',
    url='http://github.com/nukesor/pueue',
    license='MIT',
    install_requires=[
        'terminaltables>=1.0.2',
        'daemonize>=2.3.1'
    ],
    classifiers=[
        'License :: OSI Approved :: MIT License',
        'Programming Language :: Python :: 3.4',
        'Programming Language :: Python :: 3.5',
        'Environment :: Console'
    ],
    use_2to3=True,
    packages=find_packages(),
    entry_points={
            'console_scripts': [
                'pueue=pueue:main'
            ]
    })
