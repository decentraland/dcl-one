"""Responses from the GitHub CLI consumed by the cache cleanup workflow."""
import json
import re
from subprocess import CompletedProcess


def cache_cleanup_api(pages, states=None):
    states = states or {}

    def run(args, *, capture_output, text):
        if args[:2] != ['gh', 'api'] or not capture_output or not text:
            raise AssertionError(f'Unexpected command: {args}')
        if args[-1] == 'rate_limit':
            body = {'resources': {'core': {'remaining': 1000}}}
        elif any(arg.endswith('/actions/caches') for arg in args):
            body = pages
        elif args[2] == 'graphql':
            query = next(arg for arg in args if arg.startswith('query='))
            numbers = re.findall(r'p([0-9]+): pullRequest\(number: \1\)', query)
            if not numbers:
                raise AssertionError(f'Missing pull requests in query: {query}')
            body = {'data': {'repository': {
                f'p{number}': {'state': states.get(number, 'OPEN')} for number in numbers
            }}}
        elif args[2:4] == ['--method', 'DELETE']:
            return CompletedProcess(args, 0, stdout='', stderr='')
        else:
            raise AssertionError(f'Unexpected API call: {args}')
        return CompletedProcess(args, 0, stdout=json.dumps(body), stderr='')

    return run
