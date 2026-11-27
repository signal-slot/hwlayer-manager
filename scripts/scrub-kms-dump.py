#!/usr/bin/env python3
"""Scrub a KMS props/probe dump of blob contents and board identifiers.

Usage: scrub-kms-dump.py [--hostname NAME[,NAME...]]... [FILE]

Reads FILE (or stdin) and writes the scrubbed text to stdout; a summary of
the replacements goes to stderr. Exit 0 ok, 2 usage error. See
scripts/scrub-kms-dump-README.md.
"""
import argparse
import ipaddress
import re
import sys
from collections import Counter

HEX_LINE = re.compile(
    r"^(?P<indent>\s*)(?:[0-9A-Fa-f]{2,8}:?\s+)?(?P<data>(?:[0-9A-Fa-f]{2}[ :]?)+)\s*$"
)
HEX_INLINE = re.compile(r"(?<![0-9A-Za-z])(?:[0-9A-Fa-f]{2} ?){15,}[0-9A-Fa-f]{2}(?![0-9A-Za-z])")
MAC = re.compile(
    r"(?<![0-9A-Fa-f:-])(?:[0-9A-Fa-f]{2}([:-]))(?:[0-9A-Fa-f]{2}\1){4}[0-9A-Fa-f]{2}(?![0-9A-Fa-f:-])"
)
IPV6 = re.compile(r"(?<![0-9A-Za-z:.])[0-9A-Fa-f:]*:[0-9A-Fa-f:]*:[0-9A-Fa-f:.]*(?![0-9A-Za-z:])")
IPV4 = re.compile(r"(?<![0-9.])(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9])")
TTY = re.compile(r"/dev/(?:tty(?!RPMSG)[A-Za-z0-9_]\S*|serial/by-id\S*)")
USERHOST = re.compile(r"(?<![A-Za-z0-9_.<-])[A-Za-z0-9_][A-Za-z0-9._-]*@[A-Za-z][A-Za-z0-9.-]*")
USERHOST_KEEP = re.compile(r"git@git(?:hub|lab)\.com|noreply|@example\.(?:com|org|net|invalid)|user@host")
SERIAL = re.compile(
    r"(?i)(serial(?:[ _-]?(?:number|no\.?|num))?[ \t]*[:=]?[ \t]*)([^\s,;:=][^\s,;]*)"
)
CRED_VALUE = re.compile(
    r"(?i)\b(password|passwd|token|secret|api_key)([ \t]*=[ \t]*[\"']?)(?!<|\$|\{)[A-Za-z0-9+/_.-][^\s\"']*"
)


def valid_ipv6(tok):
    try:
        ipaddress.IPv6Address(tok)
        return True
    except ValueError:
        return False


def hex_digits(s):
    return len(re.sub(r"[^0-9A-Fa-f]", "", s))


class Scrubber:
    def __init__(self, hostnames):
        self.count = Counter()
        self.bytes_removed = 0
        self.hosts = None
        if hostnames:
            alts = "|".join(re.escape(h) for h in sorted(hostnames, key=len, reverse=True))
            self.hosts = re.compile(r"(?<![A-Za-z0-9-])(?:%s)(?![A-Za-z0-9-])" % alts, re.I)

    def sub(self, kind, pattern, repl, text):
        def f(m):
            r = repl(m) if callable(repl) else repl
            if r is None:
                return m.group(0)
            self.count[kind] += 1
            return r

        return pattern.sub(f, text)

    def blob(self, n):
        self.count["blob"] += 1
        self.bytes_removed += n
        return "<blob %d bytes removed>" % n

    def line(self, s):
        s = self.sub("blob", HEX_INLINE, lambda m: self.blob(hex_digits(m.group(0)) // 2), s)
        s = self.sub("mac", MAC, "<mac>", s)
        s = self.sub("ipv6", IPV6, lambda m: "<ipv6>" if valid_ipv6(m.group(0)) else None, s)
        s = self.sub("ipv4", IPV4, "<ipv4>", s)
        s = self.sub("tty", TTY, "<tty-path>", s)
        s = self.sub(
            "user@host",
            USERHOST,
            lambda m: None if USERHOST_KEEP.search(m.group(0)) else "<user@host>",
            s,
        )
        s = self.sub("serial", SERIAL, lambda m: None if m.group(2).startswith("<") else m.group(1) + "<serial>", s)
        s = self.sub("secret", CRED_VALUE, lambda m: m.group(1) + m.group(2) + "<redacted>", s)
        if self.hosts:
            s = self.sub("hostname", self.hosts, "<host>", s)
        return s

    def run(self, text):
        out = []
        pending = None  # [indent, bytes, ending] of a block of hex-only lines
        for raw in text.splitlines(keepends=True):
            body = raw.rstrip("\r\n")
            ending = raw[len(body):]
            m = HEX_LINE.match(body)
            if m and hex_digits(m.group("data")) >= 16:
                n = hex_digits(m.group("data")) // 2
                if pending:
                    pending[1] += n
                    pending[2] = ending
                else:
                    pending = [m.group("indent"), n, ending]
                continue
            if pending:
                out.append(pending[0] + self.blob(pending[1]) + pending[2])
                pending = None
            out.append(self.line(body) + ending)
        if pending:
            out.append(pending[0] + self.blob(pending[1]) + pending[2])
        return "".join(out)


def main(argv):
    ap = argparse.ArgumentParser(
        description="Scrub EDID/blob contents and board identifiers from a KMS dump."
    )
    ap.add_argument("--hostname", action="append", default=[], metavar="NAME[,NAME...]",
                    help="host name(s) to replace by <host> (repeatable)")
    ap.add_argument("file", nargs="?", help="dump file (default: stdin)")
    args = ap.parse_args(argv)
    hosts = [h for item in args.hostname for h in item.split(",") if h]
    try:
        if args.file:
            with open(args.file, "rb") as f:
                data = f.read()
        else:
            data = sys.stdin.buffer.read()
    except OSError as e:
        print("scrub-kms-dump: %s" % e, file=sys.stderr)
        return 2
    sc = Scrubber(hosts)
    out = sc.run(data.decode("utf-8", errors="surrogateescape"))
    sys.stdout.buffer.write(out.encode("utf-8", errors="surrogateescape"))
    sys.stdout.buffer.flush()
    total = sum(sc.count.values())
    parts = ", ".join("%s %d" % (k, v) for k, v in sorted(sc.count.items())) or "none"
    print("scrub-kms-dump: %d replacements (%s); %d blob bytes removed" % (total, parts, sc.bytes_removed),
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
