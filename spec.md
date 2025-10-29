# Program Specification Overview

This program is a command-line tool designed to interact with the trash functionality, offering high compatibility with standard desktop environments, and implemented based on the **Freedesktop.org Trash Specification [Version 1.0](https://specifications.freedesktop.org/trash-spec/1.0/)**. It provides functions for safely moving files to the trash, permanently deleting them, listing trash contents, and restoring items.

### Terminology

*   **`$topdir`**: Refers to the root directory of a different filesystem (e.g., a partition or removable device). A trash directory specific to that filesystem will be created under this directory.
*   **`$trash`**: Denotes the root path of an individual trash directory. Examples include `$HOME/.local/share/Trash` or `$topdir/.Trash-$uid`.
*   **`$uid`**: Represents the User ID. This is used when creating user-specific directories within a shared trash directory.

### Core Features

1.  **Move (to Trash)**
    *   **Target:** Moves files and directories within the same filesystem to `$trash`. It does not support moving items between different filesystems.
    *   **Trash Management:** If a trash directory does not exist with appropriate permissions, it will be created automatically.
    *   **Duplicate Avoidance:** Mechanisms are in place to prevent name conflicts if an item with the same name already exists in the trash.
    *   **Info File Creation:** A `.trashinfo` file corresponding to the moved item will be created in the `$trash/info/` directory, recording its original path and deletion date.

2.  **Delete (from Trash)**
    *   **Target:** Permanently deletes items from trash directories located across all filesystems (data within each `$trash/files` and `.trashinfo` files within each `$trash/info`).
    *   **Trash Reconstruction:** After deletion, the `$trash/files` and `$trash/info` directories will be appropriately recreated within each trash directory.
    *   **Confirmation Feature:** Allows users to choose whether to display a confirmation prompt before deleting items or to proceed without confirmation.

3.  **List Display**
    *   **Target:** Lists the contents of trash directories across all filesystems.
    *   **Display Formats:** Supports both a simple list display (equivalent to the `ls` command) and a detailed list display (equivalent to `ls -l` command).
    *   **Customization:** Enables color-coding based on file type, with an option to disable colorization.

4.  **Restore (from Trash)**
    *   **Target:** Restores items from the trash to their original locations.
    *   **No Overwriting:** When restoring a file from the trash, if an item with the same name already exists in the destination directory, the overwrite operation will not occur, and an error will be displayed. 
    *   **Restoration Information:** Items are accurately restored to their original paths based on the information recorded in their `.trashinfo` files.
    *   **Interactive Selection:** Supports interactive selection of items to restore.

### Implementation Details Based on Freedesktop.org Trash Specification

This program is strictly implemented according to the Freedesktop.org Trash Specification, considering the following points:

*   **Trash Directory Integrity Check:** Ensures that trash directories are not symbolic links.
*   **Prohibition of Invalid Moves:** Moving the trash directory itself or items already within the trash to the trash is prohibited.
*   **Trash Directory Placement and Priority:**
    *   The primary user-specific trash directory prioritizes `$XDG_DATA_HOME/Trash` and falls back to `$HOME/.local/share/Trash` if the former does not exist. These are created automatically as needed.
    *   Trash directories located in a `$topdir` (e.g., `/` or `/mnt/usb`) are managed in the format of `$topdir/.Trash` or `$topdir/.Trash-$uid`.
        *   If `$topdir/.Trash` has already been created by an administrator, a user-specific `$topdir/.Trash/$uid` will be created and used within it.
        *   If `$topdir/.Trash` does not exist, a user-specific `$topdir/.Trash-$uid` will be created and used.
    *   Items from different filesystems are moved to the trash directory within the `$topdir` where that filesystem is mounted. However, while the Freedesktop.org specification allows for file copying between different filesystems for trashing as an option, this program, considering the performance cost, does not support trashing that involves copying across different filesystems.
*   **Trash Directory Permissions:**
    *   User-specific trash directories (`$XDG_DATA_HOME/Trash` or `$HOME/.local/share/Trash`) are created with `700` permissions.
    *   Shared trash directories (`$topdir/.Trash`) should have `1777` permissions (sticky bit set).
    *   User-specific trash directories (`$topdir/.Trash/$uid` and `$topdir/.Trash-$uid`) are primarily created with `700` permissions. If `700` cannot be set, they will be created with `1777`.

    The Freedesktop.org specification does not explicitly state that the permissions for `$topdir/.Trash/$uid` or `$topdir/.Trash-$uid` must be `700`. However, considering that the sticky bit on `$topdir/.Trash` is intended to prevent other users from deleting subdirectories (i.e., `$topdir/.Trash/$uid`) within it, `700` permissions, granting full control to the user, are deemed appropriate. If `700` permissions cannot be set, `1777` is used as a fallback for robustness. This is an independent implementation decision not explicitly mandated by the specification, but made in consideration of the behavior of other applications.

*   **Trash Directory Structure:**
    Each `$trash` directory consists of two subdirectories:
    *   **`$trash/files`:**
        *   Actual files and directories that have been trashed are stored here.
        *   Filenames within this directory are determined by the implementation to be unique and do not depend on the original filenames.
        *   The original file permissions, access times, modification times, and extended attributes (if any) are preserved.
    *   **`$trash/info`:**
        *   Contains an "information file" for every file and directory in `$trash/files`.
        *   The filename of the information file is the same as its corresponding file in `files`, with a `.trashinfo` extension appended.
        *   `.trashinfo` files are text files similar to the Desktop Entry Specification format, containing the following information:
            *   `[Trash Info]`: The first line of the file.
            *   `Path=`: The original absolute path of the file before it was trashed (a URL-escaped string in RFC 2396/3986 format).
            *   `DeletionDate=`: The date and time the file was trashed (RFC 3339 format `YYYY-MM-DDThh:mm:ss`).

*   **`directorysizes` Cache:**
    While aiming for compliance with v1.0, this program's development acknowledges that the `directorysizes` cache is the most significant change compared to previous specification versions. The specification states "SHOULD" rather than "MUST", indicating that its implementation is recommended but not a mandatory requirement. Therefore, to avoid increasing code complexity and forgoing performance optimization, the implementation of the `directorysizes` cache feature is currently deferred.
