# tt (Trash-tool) - A Modern Command-Line Trash Can

`tt` is a spec-compliant command-line trash can utility for Unix-like systems, written in Rust. It provides a safe alternative to `rm` by moving files to the trash, with features for listing, restoring, and emptying.

## Features

*   **Safe Deletion**: Move files and directories to the trash instead of permanently deleting them.
*   **Spec Compliant**: Follows the [FreeDesktop.org Trash Specification v1.0](https://specifications.freedesktop.org/trash-spec/1.0/).
    *   Generates `.trashinfo` files recording the original path and deletion time, ensuring compatibility with desktop file managers (like GNOME, KDE, etc.).
    *   This allows items trashed by `tt` to be seen and restored from your desktop's trash GUI, and vice-versa.
*   **Collision Avoidance**: Automatically renames files if an item with the same name already exists in the trash, preventing accidental overwrites.
*   **List Contents**: View trashed items in a simple grid or a detailed (`ls -l` style) format.
*   **Interactive Restore**: Restore items with a powerful and highly customizable fuzzy-finder interface. This feature is made possible by the excellent [skim](https://github.com/skim-rs/skim) library.
*   **Empty Trash**: Securely empty all trash directories with confirmation.
*   **Multi-Drive Support**: Correctly identifies the appropriate trash directory for files on different filesystems (e.g., external drives). It uses the trash can on the same device as the file being deleted, avoiding unsupported cross-device moves.

> More detailed specifications for this program can be found [here](https://github.com/saihon/trash-tool/blob/main/spec.md).

## Installation

### From Releases

Pre-compiled binaries for Linux are available on the project's [GitHub Releases page](https://github.com/saihon/trash-tool/releases). This is the easiest way to get started.

1.  Navigate to the latest release.
2.  Download the `.zip` archive.
3.  Extract the `tt` binary from the archive.
4.  Move the binary to a directory in your system's `PATH` (e.g., `/usr/local/bin` or `~/.local/bin`).

```sh
# Example steps:
unzip tt-v0.0.0-linux-amd64.zip
chmod 755 tt
sudo mv tt /usr/local/bin/
```

### From Source

If you have the Rust toolchain installed, you can build and install `tt` from source.

```sh
git clone <repository-url>
cd trash-command
cargo install --path .
```

This will install the binary `tt` into your Cargo binary path (e.g., `~/.cargo/bin`).

## A Note on the Command Name `tt`

The command is named `tt` for its brevity, from "trash-tool". This short name may conflict with other command-line tools.

If you encounter a name collision, I recommend setting up a shell alias in your configuration file (e.g., `~/.bashrc`, `~/.zshrc`):

```sh
# Use 'trash-tool' as an alias for the 'tt' command
alias trash-tool="~/.local/bin/tt"
```

This allows you to use `trash-tool` to invoke this program without affecting other tools.

## Usage

Here is a summary of all available command-line options.

```
tt [OPTIONS] [FILES]...
```

### Actions

You can specify one of the following actions. If no action or file is provided, `-d` (display) is the default.
*   `[FILES]...`: One or more files or directories to move to the trash.
*   `-d, --display`: Display the contents of the trash directories in a grid.
*   `-l, --long`: Display trash contents in a detailed, long format (like `ls -l`).
*   `-e, --empty`: Empty each trash can after confirmation.
*   `-r, --restore`: Open an interactive fuzzy-finder to restore items from the trash.

### General Options

*   `-y, --no-confirm`: Automatically answer "yes" to confirmation prompts (e.g., for emptying the trash).
*   `--color <WHEN>`: Controls when to use colors. Possible values: `auto` (default), `always`, `never`.
*   `-h, --help`: Print help information.
*   `-V, --version`: Print version information.

### Interactive UI Options (for `restore`)
The restore action (`-r` or `--restore`) is powered by `skim` and can be customized using `skim`'s own command-line options. These options should be passed after a `ui` subcommand.

> For a detailed list of these options, see the **Configuration** section below.

### Trashing Files

*   To move one or more files/directories to the trash:
    ```sh
    tt file.txt "a folder with spaces" directory/ *.glob
    ```

### Listing Trash Contents

*   To display the contents of the trash:
    ```sh
    tt
    # or
    tt -d
    ```
*   To list contents in a detailed, long format (similar to `ls -l`):
    ```sh
    tt -l
    ```

### Restoring Items from Trash

*   To open the interactive restore interface:
    ```sh
    tt -r
    ```

### Emptying the Trash

*   To empty all trash directories with a confirmation prompt:
    ```sh
    tt -e
    ```
*   To empty the trash without any confirmation:
    ```sh
    tt -y
    # or
    tt -ey
    ```
*   Use in conjunction with content display:
    ```sh
    tt -de
    # or
    tt -le
    ```

## Configuration

The interactive restore UI is highly customizable through command-line options or the `TRASH_TOOL_OPTIONS` environment variable. Command-line options will always override settings from the environment variable.

### Environment Variable

For persistent settings, it is recommended to use the `TRASH_TOOL_OPTIONS` environment variable. You can add this to your shell's configuration file (e.g., `~/.bashrc`, `~/.zshrc`).

**Example for your `~/.bashrc`:**

```sh
# Example custom settings for the 'tt' command's restore UI
export TRASH_TOOL_OPTIONS="--multi --layout reverse --height 50%"
```

**Example for preview:**

~/preview.sh
```sh
bat "${1#* <= }"
```

~/.bashrc
```sh
export TRASH_TOOL_OPTIONS="--preview '~/preview.sh {}'"
```

### Command-Line Options

You can also specify `ui` options directly on the command line. These will override any settings from the environment variable.

```sh
# Override the height for a single run
tt -r ui --height 30%
```

### Available UI Options

You can see `tt ui --help` for all details.

## License

This project is licensed under the MIT License.
