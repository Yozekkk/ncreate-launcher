"""Read official metadata, verify SHA-1, and record every offered release's Java requirement."""
import concurrent.futures
import hashlib
import json
import pathlib
import urllib.request


def fetch(url):
	with urllib.request.urlopen(url, timeout=30) as response:
		return response.read()


def inspect(version):
	data = fetch(version['url'])
	assert hashlib.sha1(data).hexdigest() == version['sha1'], version['id']
	metadata = json.loads(data)
	return {'minecraft': version['id'], 'javaVersion': metadata.get('javaVersion'),
		'required': metadata.get('javaVersion', {}).get('majorVersion', 8),
		'url': version['url'], 'sha1': version['sha1']}


manifest = json.loads(fetch('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
releases = [v for v in manifest['versions'] if v['type'] == 'release']
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
	mapping = list(executor.map(inspect, releases))
path = pathlib.Path('packages/launcher-core/tests/fixtures/java-requirements.json')
path.write_text(json.dumps(mapping, ensure_ascii=False, indent='\t') + '\n')
print(f'Verified {len(mapping)} official release metadata documents; majors {sorted(set(v["required"] for v in mapping))}')
for version in ['1.16.5', '1.17', '1.18', '1.20.4', '1.20.5', '1.21.1']:
	print(next(v for v in mapping if v['minecraft'] == version))
