Imported shell_OS into Pasta interpreter.

Next steps:
- Adapt or wrap run_cli to accept Pasta Environment/Executor types.
- Hook into executor.rs by adding a wrapper method that calls the shell entrypoint.
- Resolve any name collisions and update module paths as needed.

Backups of overwritten files are in: /home/travis/.merge_shell_backups/backup_1772658328
