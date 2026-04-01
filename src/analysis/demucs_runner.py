"""Entry point for PyInstaller-frozen demucs binary."""
import os
import certifi

# PyInstaller-frozen binaries lose access to the system certificate store.
# Point Python's SSL to certifi's bundled CA bundle before anything else loads.
os.environ.setdefault("SSL_CERT_FILE", certifi.where())
os.environ.setdefault("REQUESTS_CA_BUNDLE", certifi.where())

from demucs.__main__ import main

if __name__ == "__main__":
    main()
