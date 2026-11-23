"""A stand-in for the `gh` calls the closed-PR cache sweep makes, all through `subprocess.run`."""
import json
import re
import subprocess
import textwrap
from pathlib import Path

CLEANUP = Path(__file__).parents[2] / '.github/workflows/closed-pr-cache.yml'


def sweep_script():
    """The sweep's Python, as the workflow runs it."""
    source = CLEANUP.read_text()
    return textwrap.dedent(source.split("python3 - <<'PY'\n", 1)[1].rsplit('\n          PY', 1)[0])


class FakeGh:
    """Answers the rate-limit read, the cache listing, the batched GraphQL PR states and the deletions.

    `pages` is the listing as `--paginate --slurp` returns it, `closed` the PR numbers (as strings) whose
    state is closed or merged, `remaining` the core budget the rate-limit read reports. Every call is kept
    in `calls`; anything else fails the test.
    """

    def __init__(self, pages, closed=(), remaining=1000, listing_error=None):
        self.pages = pages
        self.closed = {str(number) for number in closed}
        self.remaining = remaining
        self.listing_error = listing_error
        self.calls = []

    def __call__(self, command, **kwargs):
        self.calls.append(command)
        assert command[0] == 'gh', command
        args = command[1:]
        if args[:2] == ['api', 'rate_limit']:
            return self._ok({'resources': {'core': {'remaining': self.remaining}}})
        if args[:2] == ['api', 'graphql']:
            query = next(arg for arg in args if arg.startswith('query='))
            numbers = re.findall(r'p([0-9]+): pullRequest', query)
            return self._ok({'data': {'repository': {
                f'p{number}': {'state': 'MERGED' if number in self.closed else 'OPEN'} for number in numbers}}})
        if self._deletion(args):
            return subprocess.CompletedProcess(command, 0, stdout='', stderr='')
        if self._listing(args):
            if self.listing_error:
                return subprocess.CompletedProcess(command, 1, stdout='', stderr=self.listing_error)
            return self._ok(self.pages)
        raise AssertionError(f'unexpected gh call: {command}')

    @staticmethod
    def _ok(body):
        return subprocess.CompletedProcess(['gh'], 0, stdout=json.dumps(body), stderr='')

    @staticmethod
    def _deletion(args):
        return '--method' in args and args[args.index('--method') + 1] == 'DELETE'

    @classmethod
    def _listing(cls, args):
        return not cls._deletion(args) and any(arg.endswith('/actions/caches') for arg in args)

    def listings(self):
        return [call for call in self.calls if self._listing(call[1:])]

    def state_lookups(self):
        return [call for call in self.calls if call[1:3] == ['api', 'graphql']]

    def deleted(self):
        return [call[-1] for call in self.calls if self._deletion(call[1:])]
