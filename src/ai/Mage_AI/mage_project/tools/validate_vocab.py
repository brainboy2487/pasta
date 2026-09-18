#!/usr/bin/env python3
"""Simple JSONL validator for Mage vocabulary files.

Usage: python3 tools/validate_vocab.py data/vocab_examples.jsonl

This script performs lightweight checks that mirror the project's schema
without depending on external libraries.
"""
import sys, json

def load_schema(path):
    with open(path, 'r', encoding='utf-8') as f:
        return json.load(f)

def validate_entry(e, schema):
    # Required fields
    for k in schema.get('required', []):
        if k not in e:
            return False, f"missing required field: {k}"
    # Basic types
    if not isinstance(e['id'], str):
        return False, 'id must be string'
    if not isinstance(e['key'], str):
        return False, 'key must be string'
    if not isinstance(e['responses'], list) or len(e['responses']) == 0:
        return False, 'responses must be non-empty array'
    if 'type' in e and e['type'] not in schema['properties']['type']['enum']:
        return False, f"invalid type: {e.get('type')}"
    # length limits
    if len(e['key']) > 128:
        return False, 'key too long'
    for r in e['responses']:
        if not isinstance(r, str):
            return False, 'responses must be strings'
        if len(r) > 1000:
            return False, 'response too long'
    return True, ''

def main():
    if len(sys.argv) < 2:
        print('usage: validate_vocab.py <file.jsonl>')
        return 2
    schema = load_schema('data/vocab_schema.json')
    path = sys.argv[1]
    seen_ids = set()
    ok = True
    with open(path, 'r', encoding='utf-8') as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception as ex:
                print(f'{path}:{i}: invalid json: {ex}')
                ok = False
                continue
            valid, msg = validate_entry(obj, schema)
            if not valid:
                print(f'{path}:{i}: validation failed: {msg}')
                ok = False
                continue
            if obj['id'] in seen_ids:
                print(f'{path}:{i}: duplicate id: {obj["id"]}')
                ok = False
            seen_ids.add(obj['id'])
    if ok:
        print(f'{path}: OK ({len(seen_ids)} entries)')
        return 0
    else:
        return 1

if __name__ == '__main__':
    sys.exit(main())
