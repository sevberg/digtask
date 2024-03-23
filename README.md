# Dig

**Dig** is a composable task executor which dig into whatever you need automated. It...

## Quirks

* At each level, variables are evaluated before "env" and "dir", so:
  * The global vars should not any references to environment variables which are not externally visible
  * Task vars will only be composable from it's parent's envs (for the main task, this means the global envs, but for subtask this refers to their parent)