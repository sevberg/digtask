# Dig

**Dig** is a composable task executor which dig into whatever you need automated. It...

## Quirks

* At each level, variables are evaluated before "env" and "dir", so:
  * The global vars should not any references to environment variables which are not externally visible
  * Task vars will only be composable from it's parent's envs (for the main task, this means the global envs, but for subtask this refers to their parent)

## WIP Features

* **improve errors** Root-out anyhow, and use thiserror+enums instead
* **on_error**: When a task fails, give the option of crashing (default), ignoring, or running another task
* **includes**: Allow config files to be composed of other config files (i.e. 'namespaces')
* **dot_env**:  Allow importing environment variables from a file BEFORE the global 'vars' are evaluated
* **dig out**: CLI command to run a task based on a specified output
* **dig list**: CLI command to list available tasks
  * Add task config to allow hiding tasks
* **dig summary [TASK]**: A CLI command to print a description of a specified task
* **run_if and skip_if**: Change 'if' configs to 'run_if', and implement the inverse 'skip_if'
* **prevent duplication**: Keep track of executed tasks, and prevent duplicate runs (unless specifically allowed)