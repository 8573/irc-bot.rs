irc-bot.rs [![Docs.rs][b-docs]][docs] [![Crates.io][b-crate]][crate]
===

A library for writing [Internet Relay Chat (IRC) bots] in the programming
language [Rust], additionally providing a pre-configured bot for immediate
use.

[Internet Relay Chat (IRC) bots]: <https://en.wikipedia.org/wiki/IRC_bot>
[Rust]: <https://www.rust-lang.org>

What documentation there is should be available [on Docs.rs][docs].

[docs]: <https://docs.rs/irc-bot>
[b-docs]: <https://docs.rs/irc-bot/badge.svg>

[crate]: <https://crates.io/crates/irc-bot>
[b-crate]: <https://img.shields.io/crates/v/irc-bot.svg>


Quick-start
---

To use this library without writing one's own bot with it, run the provided
program `src/bin/egbot.rs`:

    $ # For most people:
    $ cargo run
    $ # For NixOS users:
    $ make run

The name `egbot` is derived from ["e.g."], which means "for example", and is
also a pun on the name of [Eggdrop], an old IRC bot.

["e.g."]: <https://en.wiktionary.org/wiki/e.g.>
[Eggdrop]: <https://en.wikipedia.org/wiki/Eggdrop>

The bot can be configured by editing the [YAML] file `config.yaml`. One should
at least put one's IRC nick in the `admins` field — e.g., if one's nick is
["Ferris"]:

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
[`Makefile`]: <https://github.com/8573/irc-bot.rs/blob/master/Makefile>


Supported Rust versions
---

This package supports Rust version 1.34.2 and any Rust versions
backwards-compatible therewith. This minimum supported Rust version may be
increased at any time to the latest release of Rust understood by the
maintainer(s) of this package to fix or to mitigate one or more security
vulnerabilities in the standard library or compiler output. The minimum
supported Rust version may not be increased for reasons unrelated to security.

Although increases in the minimum supported Rust version are breaking changes,
they are also, under this policy, bug-fixes, and for the purposes of [SemVer]
they will be treated as bug-fixes and not as breaking changes. The idea here
is that not upgrading Rust when a security fix is available is an
irresponsible course of (in)action that the maintainer(s) of this package wish
not to support, as confessedly doctrinairely as such a denial of support may
ignore users' reasons for not updating.

[SemVer]: <https://semver.org>

