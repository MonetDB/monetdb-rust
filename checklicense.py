#!/usr/bin/env python3

import argparse
import difflib
import fnmatch
import os
import subprocess
import sys
from typing import List, Optional, Tuple

def find_files_to_check():
    """Determine set of files that must carry a copyright notice.
    Dict file_prefixes will map those not to None but to a str indicating
    the comment style.
    """
    git_cmd = ['git',  'ls-files',  '-c',  '-m',  '-o',  '--exclude-standard']
    git_files = subprocess.check_output(git_cmd, text=True).strip().splitlines()
    rules = [
        ('*.rs', '//'),
        ('*.md', ''),
        ('*.yml', '#'),
        ('checklicense.py', ''),
        ('.gitignore', ''),
        ('Cargo.toml', ''),
        ('release.toml', ''),
        ('LICENSE', ''),
        ('TODO.org', ''),
    ]
    file_prefixes = dict()
    unknown = []
    for f in git_files:
        prefix = None
        for rule in rules:
            if fnmatch.fnmatch(f, rule[0]):
                prefix = rule[1]
                break
        if prefix:
            file_prefixes[f] = prefix
        elif prefix is None:
            unknown.append(f)
        else:
            pass
    if unknown:
        msg = 'Please add rules to categorize the following files: '
        msg += ', '.join(unknown)
        raise Exception(msg)
    return file_prefixes


HEADER = """
SPDX-License-Identifier: MPL-2.0

This Source Code Form is subject to the terms of the Mozilla Public
License, v. 2.0.  If a copy of the MPL was not distributed with this
file, You can obtain one at http://mozilla.org/MPL/2.0/.

Copyright 2024 MonetDB Foundation
"""

HEADER_LINES = HEADER.lstrip().splitlines()

def find_copyright(prefix: str, lines: List[str]) -> Optional[Tuple[int,int]]:
    # keep all comment lines with the prefix stripped, turn everything else
    # into empty lines.
    reduced_lines = '\n'.join(
        line[len(prefix):].strip() if line.startswith(prefix) else ''
        for line in lines
    )
    reduced_header = HEADER.strip()
    pos = reduced_lines.find(reduced_header)

    # print(repr(reduced_lines))
    # print(repr(reduced_header))
    # print(pos)
    if pos < 0:
        return None
    else:
        start = len(reduced_lines[:pos].splitlines())
        end = start + len(reduced_header.splitlines())
        while end < len(lines) and lines[end].strip() == '':
            end += 1
        # print((start,end))
        return (start, end)

def fix_copyright(prefix: str, old_lines: List[str]) -> List[str]:
    range = find_copyright(prefix, old_lines)
    if not range:
        if old_lines and old_lines[0].startswith('#!/'):
            range = (1,1)
        else:
            range = (0,0)
    (start, end) = range
    new_lines = old_lines[:start] + [(prefix + ' ' + line).rstrip() + '\n' for line in HEADER_LINES] + ['\n'] + old_lines[end:]
    return new_lines


if __name__ == "__main__":
    argparser = argparse.ArgumentParser()
    argparser.add_argument('FILE', nargs='*')
    exclusive_group = argparser.add_mutually_exclusive_group(required=True)
    exclusive_group.add_argument('--check', action='store_true', help='Check if all files have the copyright notice')
    exclusive_group.add_argument('--patch', action='store_true', help='Print a patch that fixes the copyright notices')
    exclusive_group.add_argument('--fix', action='store_true', help='Fix the copyright notices')
    args = argparser.parse_args()

    only = set(args.FILE)
    status = 0
    for file, prefix in find_files_to_check().items():
        if only and file not in only:
            continue
        old_lines = open(file).readlines()
        new_lines = fix_copyright(prefix, old_lines)
        if new_lines == old_lines:
            continue
        elif args.check:
            status = 1
            print(f'Copyright missing: {file}')
        elif args.patch:
            print()
            print(f'diff a/{file} b/{file}')
            patch = difflib.unified_diff(old_lines, new_lines, fromfile='a/' + file, tofile='b/' + file)
            print(''.join(patch))
        elif args.fix:
            pass
        else:
            raise Exception("huh?")
