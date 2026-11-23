"""Run the cache sweep against the GitHub CLI contract without making API calls."""
import json
import os
import re
import subprocess
import textwrap
from pathlib import Path
from unittest.mock import patch


REPOSITORY = 'decentraland/dcl-one'
WORKFLOW = Path(__file__).parents[2] / '.github/workflows/closed-pr-cache.yml'


class CacheSweepCLI:
    def __init__(self, pages, *, states=None, remaining=1000):
        self.pages = pages
        self.states = states or {}
        self.remaining = remaining
        self.listings = []
        self.lookups = []
        self.deletions = []
        self.commands = []

    def run(self, args, *, capture_output, text):
        self.commands.append(args)
        caches = f'repos/{REPOSITORY}/actions/caches'
        returncode, stderr = 0, ''
        if args == ['gh', 'api', 'rate_limit']:
            reply = {'resources': {'core': {'remaining': self.remaining}}}
        elif args[:3] == ['gh', 'api', 'graphql'] and args[3::2] == ['-f'] * 3:
            fields = dict(field.split('=', 1) for field in args[4::2])
            if (fields.get('owner'), fields.get('name')) != tuple(REPOSITORY.split('/')):
                raise AssertionError(f'Unexpected GraphQL repository: {args}')
            numbers = re.findall(r'\bp([1-9]\d*): pullRequest\(number: \1\) \{ state \}', fields['query'])
            if not numbers:
                raise AssertionError(f'No PR state aliases in query: {args}')
            self.lookups.append(numbers)
            states = {number: self.states.get(number, 'OPEN') for number in numbers}
            reply = {'data': {'repository': {
                f'p{number}': {'state': state} if state is not None else None
                for number, state in states.items()
            }}}
            if None in states.values():
                reply['errors'] = [{'type': 'NOT_FOUND', 'path': ['repository', f'p{number}']}
                                   for number, state in states.items() if state is None]
                returncode, stderr = 1, 'gh: Could not resolve to a PullRequest'
        elif args[:6] == ['gh', 'api', '--method', 'GET', '--paginate', '--slurp'] and args[6] == caches:
            self.listings.append(args)
            reply = self.pages
        elif args[:4] == ['gh', 'api', '--method', 'DELETE'] and args[4].startswith(f'{caches}/'):
            self.deletions.append(args[4])
            return subprocess.CompletedProcess(args, 0, stdout='', stderr='')
        else:
            raise AssertionError(f'Unexpected GitHub CLI command: {args}')
        return subprocess.CompletedProcess(args, returncode, stdout=json.dumps(reply), stderr=stderr)

    def sweep(self, event='schedule', number=''):
        source = WORKFLOW.read_text()
        script = textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])
        with patch.dict(os.environ, {'GITHUB_EVENT_NAME': event, 'PR_NUMBER': number,
                                     'GITHUB_REPOSITORY': REPOSITORY}), \
                patch('subprocess.run', side_effect=self.run), patch('builtins.print'):
            exec(compile(script, str(WORKFLOW), 'exec'), {})
