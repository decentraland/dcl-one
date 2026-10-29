"""Evaluate the rendered Nginx `map` blocks the way Nginx resolves a request's host rewrite."""
import re

TOKEN = re.compile(r'"((?:[^"\\]|\\.)*)"|([^\s;"]+)|(;)')


def unquote(value):
    return re.sub(r'\\([\\"])', r"\1", value)


def parse(text):
    maps = {}
    for source, name, body in re.findall(r'\bmap\s+("[^"]*"|\S+)\s+\$(\w+)\s*\{(.*?)\}', text, re.S):
        entries, current = [], []
        for quoted, bare, end in TOKEN.findall(body):
            if end:
                entries.append(current)
                current = []
            else:
                current.append(unquote(quoted) if quoted or not bare else bare)
        default, exact, regex = "", {}, []
        for entry in entries:
            if entry == ["volatile"]:
                continue
            key, value = entry
            if key == "default":
                default = value
            elif key.startswith("~"):
                flags = re.I if key.startswith("~*") else 0
                pattern = re.sub(r"\(\?<(\w+)>", r"(?P<\1>", key.lstrip("~*"))
                regex.append((re.compile(pattern, flags), value))
            else:
                exact[key] = value
        maps[name] = (source.strip('"'), default, exact, regex)
    return maps


class Request:
    """Resolve map variables for one request; named captures become variables, as in Nginx."""

    def __init__(self, maps, **variables):
        self.maps, self.vars = maps, dict(variables)

    def interpolate(self, value):
        return re.sub(r"\$(\w+)", lambda m: self[m[1]], value)

    def __getitem__(self, name):
        if name in self.vars:
            return self.vars[name]
        source, default, exact, regex = self.maps[name]
        key = self.interpolate(source)
        if key in exact:
            return self.interpolate(exact[key])
        for pattern, value in regex:
            if match := pattern.search(key):
                self.vars.update({k: v for k, v in match.groupdict().items() if v is not None})
                return self.interpolate(value)
        return self.interpolate(default)


def route(text, host, request_uri):
    """Return the URI that selects a location and the forwarded `x-original-path`."""
    request = Request(parse(text), host=host, request_uri=request_uri, uri=request_uri.split("?")[0])
    uri = request["gateway_uri"] or request["uri"]
    return uri, request["original_path"]
