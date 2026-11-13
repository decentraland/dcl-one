import re

PROSE_TREES = ('docs/**', 'services/catalyst/docs/**', 'rendering/bevy-explorer/docs/**')
DOCUMENTATION_PATHS = ('!**/*.md', *('!' + path for path in PROSE_TREES), 'apps/sites/content/**')
DEVELOPMENT_DOCUMENTATION_PATHS = (*DOCUMENTATION_PATHS, 'services/catalyst/**/tests/fixtures/**',
                                  'library/**/tests/fixtures/**')


def glob_regex(pattern):
    out, i = '', 0
    while i < len(pattern):
        if pattern.startswith('**/', i):
            out += '(?:.*/)?'
            i += 3
        elif pattern.startswith('**', i):
            out += '.*'
            i += 2
        elif pattern[i] == '*':
            out += '[^/]*'
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile(out + r'\Z')


def matching_paths(changed, paths):
    if paths is None:
        return list(changed)
    regexes = [(p.startswith('!'), glob_regex(p.removeprefix('!'))) for p in paths]
    matches = []
    for path in changed:
        included = False
        for excluded, regex in regexes:
            if regex.match(path):
                included = not excluded
        if included:
            matches.append(path)
    return matches


def non_documentation(changed):
    return matching_paths(changed, ('**', *DOCUMENTATION_PATHS, '**/tests/fixtures/**'))
