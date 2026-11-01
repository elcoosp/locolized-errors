#!/bin/bash
# ==============================================================================
# Simple Bash 3.2 compatible script for macOS
# Commits every changed file individually with conventional commit messages.
# ==============================================================================

set -e

# 1. Unstage everything first. This ensures that when we commit,
# we ONLY commit the single file we are currently processing.
echo "Unstaging all files to prepare for individual commits..."
git reset

# 2. Execute the Python logic directly via heredoc
python3 - << 'EOF'
import subprocess
import sys
import os

def run_git(*args):
    """Execute a git command and return its stdout."""
    res = subprocess.run(['git'] + list(args), capture_output=True, text=True)
    if res.returncode != 0:
        # git diff returns 1 if there are no differences, which is fine
        if args[0] != 'diff' or res.returncode > 1:
            print(f"Git error: {res.stderr.strip()}")
            sys.exit(1)
    return res.stdout

def get_files():
    """Get all modified, added, and untracked files from git status."""
    out = run_git('status', '--porcelain')
    files = []
    for line in out.split('\n'):
        if not line.strip():
            continue
        # Format is "XY filename". Filename starts at index 3.
        path = line[3:]
        if ' -> ' in path:  # Handle renames
            path = path.split(' -> ')[-1]
        files.append(path)
    return files

def analyze(filepath, diff):
    """Analyze the diff to generate a conventional commit subject and body."""
    is_new = 'new file mode' in diff or '--- /dev/null' in diff
    is_deleted = 'deleted file mode' in diff

    adds = len([l for l in diff.split('\n') if l.startswith('+') and not l.startswith('+++')])
    dels = len([l for l in diff.split('\n') if l.startswith('-') and not l.startswith('---')])

    # Determine Commit Type
    if filepath.startswith('.github/'): ctype = 'ci'
    elif filepath.startswith('docs/') or filepath.endswith('.md'): ctype = 'docs'
    elif 'tests/' in filepath or filepath.endswith('.stderr'): ctype = 'test'
    elif filepath.endswith('.toml') or filepath.endswith('.lock'): ctype = 'chore'
    elif filepath.endswith('.rs'): ctype = 'feat' if is_new else 'refactor'
    elif filepath.endswith('.yml') or filepath.endswith('.yaml'): ctype = 'feat' if is_new else 'chore'
    else: ctype = 'chore'

    # Determine Scope
    scope = ""
    if 'macros' in filepath: scope = "macros"
    elif filepath.startswith('locolized-errors/src'): scope = "core"
    elif filepath.startswith('locolized-errors/tests'): scope = "tests"
    elif 'locales' in filepath: scope = "i18n"

    scope_str = f"({scope})" if scope else ""

    # Determine Subject
    filename = os.path.basename(filepath)
    name_no_ext = os.path.splitext(filename)[0].replace('_', ' ')
    action = "add" if is_new else ("remove" if is_deleted else "update")

    if filepath.endswith('.rs'): subject = f"{action} {name_no_ext} module"
    elif filepath.endswith('.md'): subject = f"{action} documentation"
    elif filepath.endswith('.yml'): subject = f"{action} {filename}"
    elif filepath.endswith('.toml'): subject = f"{action} dependencies and metadata"
    elif filepath.endswith('.stderr'): subject = f"{action} expected diagnostic for {name_no_ext}"
    else: subject = f"{action} {filename}"

    subj = f"{ctype}{scope_str}: {subject}"

    # Determine Body
    if is_new: body = f"Introduces `{filepath}` to the project."
    elif is_deleted: body = f"Removes `{filepath}` from the project."
    else: body = f"Modifies `{filepath}` with {adds} additions and {dels} deletions."

    return subj, body

def main():
    # Verify we are in a git repository
    run_git('rev-parse', '--is-inside-work-tree')

    files = get_files()
    # Filter out the diff file if it's still lying around
    files = [f for f in files if f != 'changed.txt']

    if not files:
        print("No files to commit.")
        return

    print(f"Found {len(files)} files to commit.\n")

    for f in files:
        # 1. Stage the specific file
        if os.path.exists(f):
            run_git('add', f)
        else:
            run_git('rm', f)

        # 2. Get the diff for this specific file against HEAD
        diff = run_git('diff', 'HEAD', '--', f)

        # 3. Analyze and commit
        subj, body = analyze(f, diff)
        run_git('commit', '-m', subj, '-m', body)
        print(f"✅ {subj}")

    print("\n🎉 All files committed successfully!")

if __name__ == '__main__':
    main()
EOF
