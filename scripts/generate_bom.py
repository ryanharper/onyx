import json
import uuid
from datetime import datetime, timezone

def generate_cyclonedx(metadata_file, output_file):
    with open(metadata_file, 'r') as f:
        metadata = json.load(f)

    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.4",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
            "tools": [
                {
                    "vendor": "Onyx",
                    "name": "generate_bom.py",
                    "version": "1.0.0"
                }
            ],
            "component": {
                "name": "yt-frontend",
                "version": "0.1.0",
                "type": "application"
            }
        },
        "components": []
    }

    for pkg in metadata.get('packages', []):
        component = {
            "type": "library",
            "name": pkg['name'],
            "version": pkg['version'],
        }

        if pkg.get('description'):
            component['description'] = pkg['description']

        if pkg.get('license'):
            component['licenses'] = [
                {
                    "license": {
                        "id": pkg['license']
                    }
                }
            ]
        elif pkg.get('license_file'):
             component['licenses'] = [
                {
                    "license": {
                        "name": "See license file"
                    }
                }
            ]

        bom['components'].append(component)

    with open(output_file, 'w') as f:
        json.dump(bom, f, indent=2)

if __name__ == "__main__":
    generate_cyclonedx('metadata.json', 'BOM.json')
