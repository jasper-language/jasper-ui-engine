#!/usr/bin/env python3
"""Reject backend Rust type paths in rustdoc public signatures; not a semantic/proof gate."""
import argparse
import copy
import json
from pathlib import Path

FORBIDDEN = ('blitz_', 'anyrender', 'fontique', 'parley', 'vello', 'keyboard_types', 'markup5ever')


def check(document):
    index = document['index']
    selected = {key for key, value in index.items()
                if value['crate_id'] == 0 and value['visibility'] == 'public'}
    # Follow inherited enum payloads and associated items of public traits/trait impls.
    pending = list(selected)
    while pending:
        key = pending.pop()
        item = index.get(str(key))
        if not item:
            continue
        inner = item['inner']
        children = []
        for kind in ('struct', 'enum', 'union'):
            if kind in inner:
                children.extend(inner[kind].get('impls', []))
        if 'trait' in inner:
            children.extend(inner['trait']['items'])
        if 'enum' in inner:
            children.extend(inner['enum']['variants'])
        if 'impl' in inner and inner['impl'].get('trait') is not None:
            children.extend(inner['impl']['items'])
        if 'variant' in inner:
            kind = inner['variant']['kind']
            if isinstance(kind, dict):
                children.extend(kind.get('tuple', []))
                children.extend(kind.get('struct', {}).get('fields', []))
        for child in children:
            child = str(child)
            candidate = index.get(child, {}).get('inner', {}).get('impl', {})
            # rustdoc attaches upstream blanket impls to every matching public type.
            # They are not signatures authored/exposed by this adapter.
            if candidate.get('blanket_impl') is not None:
                continue
            if child not in selected:
                selected.add(child)
                pending.append(child)
    violations = []
    expanded_aliases = set()

    def walk(value, item):
        if isinstance(value, dict):
            if 'id' in value:
                reference = str(value['id'])
                path = document['paths'].get(reference, {})
                alias = index.get(reference, {}).get('inner', {}).get('type_alias')
                if alias is not None and reference not in expanded_aliases:
                    expanded_aliases.add(reference)
                    walk(alias, item)
                parts = path.get('path', [])
                if parts and parts[0].startswith(FORBIDDEN):
                    violations.append({'item': item, 'backend_type': '::'.join(parts)})
            for nested in value.values():
                walk(nested, item)
        elif isinstance(value, list):
            for nested in value:
                walk(nested, item)

    for key in selected:
        item = index.get(str(key))
        if item:
            walk(item['inner'], item['name'])
    return {'kind': 'rustdoc_public_type_boundary_only', 'current': not violations,
            'violations': violations,
            'limitations': ['Checks backend type paths, not strings, runtime behavior or correctness.',
                            'Does not freeze API names or accept any Rust implementation.',
                            'Upstream generic blanket impls attached by rustdoc are excluded; explicit impls are checked.',
                            'Re-exported Jasper dependency definitions require their own crate audit.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('rustdoc_json', type=Path)
    args = parser.parse_args()
    document = json.loads(args.rustdoc_json.read_text())
    result = check(document)
    # Deliberately expose a backend type through a real public function signature.
    negative = copy.deepcopy(document)
    function = next(item for item in negative['index'].values()
                    if item['crate_id'] == 0 and item['visibility'] == 'public' and 'function' in item['inner'])
    negative['paths']['boundary-negative-control'] = {'crate_id': 999, 'path': ['blitz_dom', 'BaseDocument'], 'kind': 'struct'}
    function['inner']['function']['sig']['output'] = {'resolved_path': {'path': 'BaseDocument', 'id': 'boundary-negative-control', 'args': None}}
    result['backend_return_negative_rejected'] = not check(negative)['current']
    alias_id = 'boundary-private-alias'
    backend = copy.deepcopy(function['inner']['function']['sig']['output'])
    negative['index'][alias_id] = {'crate_id': 0, 'visibility': 'default', 'name': 'HiddenBackendAlias',
        'inner': {'type_alias': {'type': backend, 'generics': {'params': [], 'where_predicates': []}}}}
    negative['paths'][alias_id] = {'crate_id': 0, 'path': ['jasper_ui_blitz_host_probe', 'HiddenBackendAlias'], 'kind': 'type_alias'}
    function['inner']['function']['sig']['output'] = {'resolved_path': {'path': 'HiddenBackendAlias', 'id': alias_id, 'args': None}}
    result['private_backend_alias_negative_rejected'] = not check(negative)['current']

    print(json.dumps(result, indent=2))
    return 0 if result['current'] and result['backend_return_negative_rejected'] and result['private_backend_alias_negative_rejected'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
