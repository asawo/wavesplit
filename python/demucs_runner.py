"""Entry point for PyInstaller-frozen demucs binary."""
import multiprocessing
import os
import certifi

# Must be called before any other code when frozen with PyInstaller.
# PyTorch uses multiprocessing internally; without this, spawned child processes
# re-invoke the frozen binary with Python interpreter flags (-B -S -I -c) that
# demucs' argparser doesn't recognise.
multiprocessing.freeze_support()

# PyInstaller-frozen binaries lose access to the system certificate store.
# Point Python's SSL to certifi's bundled CA bundle before anything else loads.
os.environ.setdefault("SSL_CERT_FILE", certifi.where())
os.environ.setdefault("REQUESTS_CA_BUNDLE", certifi.where())

from demucs.__main__ import main

if __name__ == "__main__":
    main()
