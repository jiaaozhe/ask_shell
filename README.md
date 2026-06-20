# ask

`ask` is a Rust CLI that turns a natural language request into a shell command.
It shows the command first, lets you edit it or give feedback, and only runs it
after approval.

It can also ask short follow-up questions or run approved read-only inspect
commands when local context is needed.

## Features

- Generate one-line shell commands from natural language
- Review actions with an interactive menu: `Run`, `Edit`, `Give Feedback`, `Cancel`
- Approve inspect commands before they run
- Show inspect output before sending it back to the model
- Inject OS, architecture, kernel, shell path, and shell version into the prompt
- Support OpenAI-compatible and Anthropic-compatible APIs
- Use structured JSON output when the provider supports it, with fallback parsing
- Optional shell integration so executed commands appear in shell history

## Install

Build a release binary:

```sh
cargo build --release
```

Run it directly:

```sh
./target/release/ask --help
```

Or install it into your Cargo bin directory:

```sh
cargo install --path .
```

## Configure

On first use, `ask` will guide you through configuration if no config file
exists.

You can also configure manually:

```sh
ask config init
```

Config is saved as TOML:

```sh
ask config path
```

Example:

```toml
provider = "openai"
model = "gpt-4.1-mini"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
temperature = 0.2
```

Anthropic example:

```toml
provider = "anthropic"
model = "claude-3-5-sonnet-latest"
base_url = "https://api.anthropic.com"
api_key = "sk-ant-..."
temperature = 0.2
```

Show the current config with the API key masked:

```sh
ask config show
```

## Usage

Ask for a command:

```sh
ask "find the 10 largest files under the current directory"
```

Or start with an interactive prompt:

```sh
ask
```

When a command is ready, choose an action:

```text
Command:
find . -type f -exec du -h {} + | sort -hr | head -n 10

Action
> Run
  Edit
  Give Feedback
  Cancel
```

Use `Give Feedback` when the command is not what you wanted. `ask` will continue
the conversation and generate a revised command.

## Shell History Integration

By default, commands are executed by the `ask` process, so they may not appear in
your current shell history. To make approved commands run in your current shell,
add shell integration.

This is required for immediate history support. A standalone child process cannot
modify the in-memory history of its parent shell.

For zsh:

```sh
eval "$(ask shell-init zsh)"
```

For bash:

```sh
eval "$(ask shell-init bash)"
```

Add the line to `~/.zshrc` or `~/.bashrc`.

Example:

```sh
echo 'eval "$(ask shell-init zsh)"' >> ~/.zshrc
source ~/.zshrc
```

For bash:

```sh
echo 'eval "$(ask shell-init bash)"' >> ~/.bashrc
source ~/.bashrc
```

Verify that the integration is active:

```sh
type ask
```

It should report that `ask` is a shell function. If it still points to the binary
path, the integration is not active.

With shell integration enabled, `ask "..."` records the approved command in the
current shell history. Running `ask` with no arguments still opens the normal
interactive prompt.

## Notes

- Supported shells: `bash`, `zsh`
- Config path: `~/.config/ask/config.toml`
- API keys are stored in the local config file as plain text
- UI output is written to stderr; command output modes use stdout where needed
