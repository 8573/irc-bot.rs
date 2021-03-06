irc-bot.rs
===

[![Docs.rs][b-docs]][docs]
[![Crates.io][b-crate]][crate]
[![GitLab CI status][b-CI-GitLab]][CI-GitLab]
[![Travis CI status][b-CI-Travis]][CI-Travis]

`irc-bot` is a library for writing [Internet Relay Chat (IRC) bots] in the
programming language [Rust], additionally providing a pre-configured bot for
immediate use.

This project is hosted both [on GitHub] and [on GitLab]; the repositories are
synchronized automatically.

If you use, or are interested in using, this library, you may post in one of
the support tickets that have been opened [on GitHub][users-ticket-GH] and [on
GitLab][users-ticket-GL] in which for people to note their interest in this
library so that this library's maintainers can survey them as to, notify them
of, and/or help them adjust to changes in the API of this library, and perhaps
provide other support.

[CI-GitLab]: <https://gitlab.com/c74d/irc-bot.rs/pipelines>
[CI-Travis]: <https://travis-ci.org/8573/irc-bot.rs>
[Internet Relay Chat (IRC) bots]: <https://en.wikipedia.org/wiki/IRC_bot>
[Rust]: <https://www.rust-lang.org>
[b-CI-GitLab]: <https://gitlab.com/c74d/irc-bot.rs/badges/dev/pipeline.svg>
[b-CI-Travis]: <https://api.travis-ci.org/8573/irc-bot.rs.svg?branch=dev>
[b-crate]: <https://img.shields.io/crates/v/irc-bot.svg>
[b-docs]: <https://docs.rs/irc-bot/badge.svg>
[crate]: <https://crates.io/crates/irc-bot>
[docs]: <https://docs.rs/irc-bot>
[on GitHub]: <https://github.com/8573/irc-bot.rs>
[on GitLab]: <https://gitlab.com/c74d/irc-bot.rs>
[users-ticket-GH]: <https://github.com/8573/irc-bot.rs/issues/50>
[users-ticket-GL]: <https://gitlab.com/c74d/irc-bot.rs/issues/1>


Documentation
---

What documentation there is for the versions of this crate published to
Crates.io should be available [on Docs.rs][docs]. Also available is
[documentation for the development version][docs-dev] of this crate, i.e., for
the latest build of this crate from the Git branch `dev`.

[docs-dev]: <https://c74d.gitlab.io/irc-bot.rs/dev/doc/irc_bot/>


Contributing
---

Please see the file [`CONTRIBUTING.md`]. Please note that the said file
contains some terms, particularly concerning licensing of contributions, to
which one must agree to contribute to this project.

[`CONTRIBUTING.md`]: <https://github.com/8573/irc-bot.rs/blob/dev/CONTRIBUTING.md>


Licensing
---

All parts of this crate are available under the standard open-source software
licence or licences specified in the file [`Cargo.toml`]. From the earliest
release of this crate, this has been [the Apache License, version 2.0].

Some portions of this crate may be available under other licences in addition,
but, in any such case, one may opt to accept such portions solely under the
crate-wide licence (or any of the crate-wide licences, if there are multiple
crate-wide licences).

[`Cargo.toml`]: <https://github.com/8573/irc-bot.rs/blob/dev/Cargo.toml>
[the Apache License, version 2.0]: <https://spdx.org/licenses/Apache-2.0.html>


Quick-start
---

To use this library without writing one's own bot with it, one can run the
provided program `src/bin/egbot.rs`:

    $ # For most people:
    $ cargo run
    $ # For NixOS users:
    $ make run

The name `egbot` is derived from ["e.g."], which means "for example", and is
also a pun on the name of [Eggdrop], an old IRC bot.

["e.g."]: <https://en.wiktionary.org/wiki/e.g.>
[Eggdrop]: <https://en.wikipedia.org/wiki/Eggdrop>

The bot can be configured by editing the [YAML] file `config.yaml`. One at
least should put one's IRC nickname ("nick") in the `admins` field — e.g., if
one's nick is ["Ferris"]:

    admins:
      - nick: Ferris

Configuration fields currently supported are as follows (with values given for
example only) (TODO: Document these in rustdoc, with rich text):

    # A string to be used as the bot's IRC nickname. This field is required.
    nickname: egbot

    # A string to be used as the bot's IRC username (which has little effect
    # in most cases). Defaults to the nickname.
    username: egbot

    # A string to be used as the bot's IRC "realname" or "GECOS string", which
    # has still less effect and is often used to display information about a
    # bot's software. Defaults to displaying information about the bot's
    # software.
    realname: 'Built with `irc-bot.rs`.'

    # A list of servers to which the bot should connect on start-up.
    # Currently, only the first server will be used, and the bot will crash if
    # no servers are listed; both of these issues should be fixed at some
    # future point.
    servers:
      - name: Mozilla
        host: irc.mozilla.org
        port: 6697
        # Whether to use Transport Layer Security. Defaults to `true`.
        TLS: true
        # A list of channels that the bot should join after connecting. Note
        # that each channel's name should be wrapped in quotation marks or
        # otherwise escaped so that the '#' is not taken as the start of a
        # comment.
        channels:
          - name: '#rust-irc'

    # A list of IRC users who will be authorized to direct the bot to run
    # certain priviledged commands. For each listed user, the fields `nick`,
    # `user`, and `host` may be specified; for each of which that is
    # specified, a user will need to have a matching nickname, username, or
    # hostname (respectively) to be authorized. All the specified fields must
    # match for a user to be authorized.
    admins:
      # To be authorized as an administrator of the bot, this user will need
      # to have the nickname "Ferris", the username "~crab", and the hostname
      # "rustacean.net":
      - nick: Ferris
        user: '~crab'
        host: rustacean.net
      # To be authorized as an administrator of the bot, this user will only
      # need have the nickname "c74d":
      - nick: c74d

[YAML]: <https://en.wikipedia.org/wiki/YAML>
["Ferris"]: <http://www.rustacean.net>


Building
---

For most users, it should suffice simply to use **[Cargo]**:

    $ cargo build

[Cargo]: <http://doc.crates.io>

Users of the Linux distribution **[NixOS]** may prefer to use the provided
[`Makefile`], which wraps the tool `nix-shell`:

    $ make build

[NixOS]: <https://nixos.org>
[`Makefile`]: <https://github.com/8573/irc-bot.rs/blob/dev/Makefile>


Supported Rust versions
---

This package supports the Rust version noted in the file [`RUST_VERSION.yaml`]
and any Rust versions backwards-compatible therewith. This minimum supported
Rust version may be increased at any time to the latest release of Rust
understood by the maintainer(s) of this package to fix or to mitigate one or
more security vulnerabilities in the standard library or compiler output. The
minimum supported Rust version may not be increased for reasons unrelated to
security.

Although increases in the minimum supported Rust version are breaking changes,
they are also, under this policy, bug-fixes, and for the purposes of [SemVer]
they will be treated as bug-fixes and not as breaking changes. The idea here
is that not upgrading Rust when a security fix is available is an
irresponsible course of (in)action that the maintainer(s) of this package wish
not to support, as confessedly doctrinairely as such a denial of support may
ignore users' reasons for not updating.

[SemVer]: <https://semver.org>
[`RUST_VERSION.yaml`]: <https://github.com/8573/irc-bot.rs/blob/dev/RUST_VERSION.yaml>
