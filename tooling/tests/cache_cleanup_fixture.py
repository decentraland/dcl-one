"""Responses from the GitHub CLI consumed by the cache cleanup workflow."""
import json
import re
from subprocess import CompletedProcess


def cache_cleanup_api(pages, states=None, remaining=(1000,), listing_error=None, states_error=None, states_error_from=0):
    """`states` maps a PR number to its state, OPEN when absent. None is a PR GitHub cannot
    resolve: gh then prints the states it found plus the error and exits 1. `remaining` is
    the core budget each rate-limit read reports in turn, the last one repeated.
    `listing_error` and `states_error` fail that call with the given stderr; `states_error`
    applies once `states_error_from` state queries have been answered."""
    states = states or {}
    budget = list(remaining)
    answered = [0]

    def run(args, *, capture_output, text):
        if args[:2] != ['gh', 'api'] or not capture_output or not text:
            raise AssertionError(f'Unexpected command: {args}')
        if args[-1] == 'rate_limit':
            body = {'resources': {'core': {'remaining': budget.pop(0) if len(budget) > 1 else budget[0]}}}
        elif any(arg.endswith('/actions/caches') for arg in args):
            # Only both flags make gh return every page as one JSON array.
            if '--paginate' not in args or '--slurp' not in args:
                raise AssertionError(f'Listing without --paginate --slurp: {args}')
            if listing_error is not None:
                return CompletedProcess(args, 1, stdout='', stderr=listing_error)
            body = pages
        elif args[2] == 'graphql':
            if states_error is not None and answered[0] >= states_error_from:
                return CompletedProcess(args, 1, stdout='', stderr=states_error)
            answered[0] += 1
            query = next(arg for arg in args if arg.startswith('query='))
            # GitHub answers under each alias the state of the PR its argument names, whatever the alias says.
            fields = re.findall(r'(p[0-9]+): pullRequest\(number: ([0-9]+)\)', query)
            if not fields:
                raise AssertionError(f'Missing pull requests in query: {query}')
            found = {alias: states.get(number, 'OPEN') for alias, number in fields}
            body = {'data': {'repository': {
                alias: {'state': state} if state else None for alias, state in found.items()
            }}}
            missing = [alias for alias, state in found.items() if state is None]
            if missing:
                body['errors'] = [{'type': 'NOT_FOUND', 'path': ['repository', alias]} for alias in missing]
                return CompletedProcess(args, 1, stdout=json.dumps(body),
                                        stderr='GraphQL: Could not resolve to a PullRequest')
        elif args[2:4] == ['--method', 'DELETE']:
            return CompletedProcess(args, 0, stdout='', stderr='')
        else:
            raise AssertionError(f'Unexpected API call: {args}')
        return CompletedProcess(args, 0, stdout=json.dumps(body), stderr='')

    return run
