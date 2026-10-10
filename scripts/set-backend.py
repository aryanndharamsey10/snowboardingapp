#!/usr/bin/env python3
"""Set the public HTTPS backend address bundled with the Pages frontend."""
import json
import sys
from pathlib import Path
from urllib.parse import urlsplit

if len(sys.argv) != 2:
    sys.exit('Usage: python3 scripts/set-backend.py https://YOUR-TUNNEL.trycloudflare.com')
url = sys.argv[1].rstrip('/')
parts = urlsplit(url)
if parts.scheme != 'https' or not parts.hostname or parts.username or parts.password or parts.path or parts.query or parts.fragment:
    sys.exit('Use a public HTTPS origin without a path, query, or credentials.')
root = Path(__file__).resolve().parents[1]
(root / 'static/config.js').write_text('// Public backend address. Empty uses the local Rust server.\nexport default ' + json.dumps({'backendUrl': url}) + ';\n')
print('Frontend backend address updated:', url)
