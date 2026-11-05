#!/usr/bin/env python3
"""Check the installed foundation contract, without an old Git checkout or draft ABI.

Compile every C function signature against its Rust export/table member; compare
installed records/constants and optionally exact shared-library exports. This is
representation/correspondence evidence, not execution or performance acceptance.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def strip_comments(text):
    return re.sub(r'/\*.*?\*/|//[^\n]*', '', text, flags=re.S)


def declarations(text):
    table = re.search(r'typedef struct ogpu_next_presentation_functions\s*\{(.*?)\}', text, re.S)
    require(table, 'missing presentation table')
    kind = r'((?:const\s+)?\w+(?:\s*\*)*)'
    direct = re.findall(kind + r'\s*\b(ogpu_next_\w+)\s*\(([^;{}]*)\)\s*;', text)
    members = re.findall(kind + r'\s*\(\*(\w+)\)\s*\(([^;]*)\)\s*;', table[1])
    result = [(name, ret.strip(), args) for ret, name, args in direct]
    result += [('presentation.' + name, ret.strip(), args) for ret, name, args in members]
    require(len({name for name, _, _ in result}) == len(result), 'duplicate operation declaration')
    return result


def rust_type(declaration, types):
    # Deliberately bounded grammar for this header. Fail closed on arrays,
    # callbacks, unknown aliases or anything that needs a richer C parser.
    array = re.fullmatch(r'(.+?)\s+(\w+)\s*\[(\d+)\]', declaration)
    if array:
        # C parameter arrays decay to a pointer; Rust preserves the documented
        # fixed extent in its pointee type. Both are one thin pointer.
        base = array[1].removeprefix('const ')
        pointer = '*const ' if array[1].startswith('const ') else '*mut '
        return pointer + '[' + rust_type(base, types) + '; ' + array[3] + ']'
    tokens = re.findall(r'[A-Za-z_]\w*|\*|\S', declaration)
    qualifiers = []
    if tokens and tokens[0] == 'const':
        qualifiers.append(tokens.pop(0))
    require(tokens and tokens[0] in types, 'unknown C type: ' + declaration)
    base = tokens.pop(0)
    result = types[base]
    while tokens and tokens[0] in ('*', 'const'):
        token = tokens.pop(0)
        if token == 'const':
            require(not qualifiers, 'unsupported qualification: ' + declaration)
            qualifiers.append(token)
        else:
            result = ('*const ' if qualifiers else '*mut ') + (result if result != '()' else 'std::ffi::c_void')
            qualifiers.clear()
    require(not qualifiers, 'qualified value argument: ' + declaration)
    require(not tokens or (len(tokens) == 1 and re.fullmatch(r'[A-Za-z_]\w*', tokens[0])),
            'unsupported C declarator: ' + declaration)
    return result


def signatures(ops, types):
    lines = ['#[allow(dead_code)] fn signatures(table: api::types::PresentationApi) {']
    for name, ret, args in ops:
        params = [] if args.strip() in ('', 'void') else [rust_type(p.strip(), types) for p in args.split(',')]
        function = 'unsafe extern "C" fn(' + ', '.join(params) + ') -> ' + rust_type(ret, types)
        if name.startswith('presentation.'):
            lines.append(f'let _: Option<{function}> = table.{name.split(".")[1]};')
        else:
            lines.append(f'let _: {function} = api::{name};')
    return lines + ['}']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path, help='also check this built library has exactly the foundation exports')
    args = parser.parse_args()
    manifest = json.loads((ROOT / 'tests/foundation-surface.json').read_text())
    header = strip_comments((ROOT / 'include/ogpu_next.h').read_text())
    version = int(re.search(r'#define OGPU_NEXT_VERSION (\d+)u', header)[1])
    require(version == manifest['revision'], 'update the reviewed surface inventory for this revision')
    ops = declarations(header)
    names = {name for name, _, _ in ops}
    require(names == set(manifest['operations']),
            f'operation drift: added {sorted(names-set(manifest["operations"]))}; removed {sorted(set(manifest["operations"])-names)}')
    sources = [ROOT / 'include/ogpu_next.h', *(ROOT / 'crates/ogpu/src/foundation').rglob('*.rs')]
    for path in sources:
        tokens = set(re.findall(r'\b\w+\b', strip_comments(path.read_text())))
        stale = tokens & set(manifest['retired_identifiers'])
        require(not stale, f'superseded foundation identifiers in {path.relative_to(ROOT)}: {sorted(stale)}')

    records = []
    types = {'size_t': 'usize', 'void': '()', 'char': 'std::ffi::c_char', 'float': 'f32', 'double': 'f64',
             **{f'{sign}int{bits}_t': f'{sign or "i"}{bits}' for sign in ('', 'u') for bits in (8, 16, 32, 64)}}
    for line in (ROOT / 'tests/abi.txt').read_text().splitlines():
        if line.startswith('next '):
            _, rust, c, *fields = line.split()
            types[c] = 'api::types::' + rust
            records.append((c, types[c], fields))
    opaque = re.findall(r'typedef struct (ogpu_next_\w+) \1\s*;', header)
    for c in opaque:
        if c not in types:
            rust = ''.join(w.title() for w in c.removeprefix('ogpu_next_').split('_'))
            types[c] = 'api::' + ('List' if rust == 'Encoder' else rust)
    for scalar, alias in re.findall(r'typedef (u?int\d+_t) (ogpu_next_\w+)\s*;', header):
        types[alias] = types[scalar]
    declared_records = set(re.findall(r'typedef (?:struct|union) \w+\s*\{.*?\}\s*(ogpu_next_\w+);', header, re.S))
    declared_records.update(re.findall(r'(?<!typedef )struct (ogpu_next_\w+)\s*\{[^{}]*\};', header))
    require(declared_records == {c for c, _, _ in records},
            f'ABI record coverage drift: {sorted(declared_records ^ {c for c, _, _ in records})}')

    c = ['#include <stdio.h>', '#include <stddef.h>', '#include "ogpu_next.h"', 'int main(void) {']
    rust = ['use ogpu::foundation as api;', *signatures(ops, types), 'fn main() {']
    for ct, rt, fields in records:
        values = [('size', f'sizeof({ct})', f'std::mem::size_of::<{rt}>()'),
                  ('align', f'_Alignof({ct})', f'std::mem::align_of::<{rt}>()')]
        for field in fields:
            cf, _, rf = field.partition('=')
            rf = rf or ('type_' if cf == 'type' else cf)
            values.append((cf, f'offsetof({ct}, {cf})', f'std::mem::offset_of!({rt}, {rf})'))
        for label, ce, rexp in values:
            c.append(f'printf("{ct}.{label}=%zu\\n", {ce});')
            rust.append(f'println!("{ct}.{label}={{}}", {rexp});')
    for ct, rt in manifest['constants'].items():
        c.append(f'printf("{ct}=%llu\\n", (unsigned long long){ct});')
        rust.append(f'println!("{ct}={{}}", api::types::{rt} as u64);')
    c.append('return 0; }')
    rust.append('}')
    # Build the real public core. This also supplies exact dependency paths for
    # rustc, without assuming the caller's target directory or building a mirror.
    built = subprocess.check_output([os.getenv('CARGO', 'cargo'), 'build', '--offline', '--no-default-features',
                                     '-p', 'ogpu', '--message-format=json'], cwd=ROOT, text=True)
    artifacts = [json.loads(line) for line in built.splitlines() if line.startswith('{')]
    library = next(Path(name) for item in artifacts if item.get('reason') == 'compiler-artifact'
                   and item['target']['name'] == 'ogpu' for name in item['filenames'] if name.endswith('.rlib'))
    cc = os.getenv('CC', 'cc')
    with tempfile.TemporaryDirectory(prefix='ogpu-foundation-audit-') as directory:
        temp = Path(directory)
        (temp / 'check.c').write_text('\n'.join(c))
        (temp / 'check.rs').write_text('\n'.join(rust))
        subprocess.run([cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-pedantic', '-I', str(ROOT / 'include'),
                        str(temp / 'check.c'), '-o', str(temp / 'check-c')], check=True)
        subprocess.run([cc, '-x', 'c++', '-std=c++17', '-Wall', '-Wextra', '-Werror', '-pedantic', '-fsyntax-only',
                        str(ROOT / 'include/ogpu_next.h')], check=True)
        subprocess.run([os.getenv('RUSTC', 'rustc'), '--edition=2021', '-Dwarnings', '-C', f'linker={cc}',
                        '--extern', f'ogpu={library}', '-L', f'dependency={library.parent / "deps"}',
                        str(temp / 'check.rs'), '-o', str(temp / 'check-rust')], check=True)
        c_values = subprocess.check_output([str(temp / 'check-c')], text=True)
        rust_values = subprocess.check_output([str(temp / 'check-rust')], text=True)
        require(c_values == rust_values, f'installed C/Rust representation mismatch:\nC:\n{c_values}\nRust:\n{rust_values}')
    if args.library:
        output = subprocess.check_output(['nm', '-D', '--defined-only', str(args.library)], text=True)
        exports = {line.split()[-1] for line in output.splitlines() if line.split()[-1].startswith('ogpu_next_')}
        expected = {name for name in names if not name.startswith('presentation.')}
        require(exports == expected, f'library export drift: {sorted(exports ^ expected)}')
    print(f'Foundation {version}: {len(ops)} C signatures match actual Rust exports/table; '
          f'{len(records)} records and {len(manifest["constants"])} constants match ({len(c_values.splitlines())} values).')
    print('Installed inventory and retired identifiers checked; no Git baseline or draft mirror used.'
          + (' Shared-library exports match.' if args.library else ''))


if __name__ == '__main__':
    main()
