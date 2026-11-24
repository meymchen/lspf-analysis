#!/usr/bin/env python3

"""check-grammar-crate
This script checks whether breaking changes could be introduced in
lspf-analysis-core after the update of a tree-sitter-grammar crate.
To do so, it compares the differences between the metrics, computed on a
chosen repository, before and after a tree-sitter-grammar update.

The fixture snapshots in crates/lspf-analysis-core/tests cover the same
ground on a handful of small files; this script is for confirming a bump
against a large real-world repository.


To compute metrics:

./check-grammar-crate.py compute-metrics -u REPO_URL -p LOCAL_DIR -g TREE_SITTER_GRAMMAR

NOTE: The compute-metrics subcommand MUST be run on a clean main branch!

To compare metrics and retrieve metrics differences and minimal tests:

1. Install json-minimal-tests from here: https://github.com/Luni-4/json-minimal-tests/releases

./check-grammar-crate.py compare-metrics -g TREE_SITTER_GRAMMAR

The clone and the metrics go in target/check-grammar-crate unless --workdir
says otherwise; both subcommands must be given the same one.

NOTE: Add the paths of the software above to the PATH environment variable!
"""

import argparse
import pathlib
import subprocess
import sys

# The clone and the metrics go here by default. It must survive between the
# two subcommands, so it can't be a fresh temporary directory, and it must not
# be a shared one like /tmp, where another user could plant a symlink at one of
# the fixed names below. target/ is already ignored by git.
DEFAULT_WORKDIR = (
    pathlib.Path(__file__).resolve().parent / "target" / "check-grammar-crate"
)
# Suffix for the directory containing the old metrics
OLD_SUFFIX = "-old"
# Suffix for the directory containing the new metrics
NEW_SUFFIX = "-new"

# Extensions parsed by each tree-sitter-grammar
EXTENSIONS = {
    "tree-sitter-cpp": ["*.cc", "*.cpp", "*.cxx", "*.hpp", "*.hxx", "*.h"],
    "tree-sitter-tsx": ["*.tsx"],
    "tree-sitter-typescript": ["*.ts"],
    "tree-sitter-rust": ["*.rs"],
    "tree-sitter-java": ["*.java"],
    "tree-sitter-python": ["*.py"],
    "tree-sitter-javascript": ["*.js", "*.mjs", "*.cjs", "*.jsx"],
}


# Run a subprocess.
def run_subprocess(cmd: str, *args: str | pathlib.Path) -> None:
    subprocess.run([cmd, *args], check=True)


# Run lspf-analysis on the chosen repository to compute metrics.
def run_metrics(
    repo_dir: pathlib.Path,
    output_dir: pathlib.Path,
    include_grammars: list[str],
) -> None:
    run_subprocess(
        "cargo",
        "run",
        "--release",
        "--package",
        "lspf-analysis",
        "--",
        "metrics",
        "--output-format=json",
        "--pr",
        "-I",
        *include_grammars,
        "--paths",
        repo_dir,
        "-o",
        output_dir,
    )


# Compute metrics before and after a tree-sitter-grammar update.
def compute_metrics(args: argparse.Namespace) -> None:

    if args.grammar not in EXTENSIONS:
        print(args.grammar, "is not a valid tree-sitter grammar")
        sys.exit(1)

    # Repository local directory
    repo_dir = args.workdir / args.path
    # Old metrics directory
    old_dir = args.workdir / (args.grammar + OLD_SUFFIX)
    # New metrics directory
    new_dir = args.workdir / (args.grammar + NEW_SUFFIX)

    # Create output directories
    old_dir.mkdir(parents=True, exist_ok=True)
    new_dir.mkdir(parents=True, exist_ok=True)

    # Skip if only new metrics are requested
    if not args.only_new:
        # Git clone the chosen repository
        print(f"Cloning {args.url} into {repo_dir}")
        run_subprocess("git", "clone", "--depth=1", args.url, repo_dir)

        # Compute old metrics
        print("\nComputing metrics before the update and saving them in", old_dir)
        run_metrics(repo_dir, old_dir, EXTENSIONS[args.grammar])

        # Create a new branch
        print("\nCreate a new branch called", args.grammar)
        run_subprocess("git", "checkout", "-B", args.grammar)

    # Compute new metrics
    print("\nComputing metrics after the update and saving them in", new_dir)
    run_metrics(repo_dir, new_dir, EXTENSIONS[args.grammar])


# Compare metrics and dump the differences whether there are some.
def compare_metrics(args: argparse.Namespace) -> None:
    # Old metrics directory
    old_dir = args.workdir / (args.grammar + OLD_SUFFIX)
    # New metrics directory
    new_dir = args.workdir / (args.grammar + NEW_SUFFIX)

    # Compare metrics directory
    compare_dir = args.workdir / (args.grammar + "-compare")

    # Create compare directory
    compare_dir.mkdir(parents=True, exist_ok=True)

    # Get JSON differences and minimal tests
    print("\nSave minimal tests in", compare_dir)
    run_subprocess("json-minimal-tests", "-o", compare_dir, old_dir, new_dir)


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="check-grammar-crate",
        description="This tool computes the metrics of a chosen repository "
        "before and after a tree-sitter grammar update.",
        epilog="The source code of this program can be found on "
        "GitHub at https://github.com/meymchen/lspf-analysis",
    )

    # Options shared by both subcommands
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument(
        "--workdir",
        "-w",
        type=pathlib.Path,
        default=DEFAULT_WORKDIR,
        help="Directory holding the clone and the metrics (default: %(default)s)",
    )

    # Subcommands parsers
    commands = parser.add_subparsers(help="Sub-command help")

    # Compute metrics command
    compute_metrics_cmd = commands.add_parser(
        "compute-metrics",
        parents=[common],
        help="Computes the metrics of a chosen repository before and after "
        "a tree-sitter grammar update.",
    )

    # Optional arguments
    compute_metrics_cmd.add_argument(
        "--only-new",
        "-n",
        action="store_true",
        help="Only compute the metrics after a tree-sitter grammar update",
    )

    # Arguments
    compute_metrics_cmd.add_argument(
        "-u",
        "--url",
        type=str,
        required=True,
        help="URL of the repository used to compute the metrics",
    )

    compute_metrics_cmd.add_argument(
        "-p",
        "--path",
        type=str,
        required=True,
        help="Path where the repository will be saved locally",
    )

    compute_metrics_cmd.add_argument(
        "-g",
        "--grammar",
        type=str,
        required=True,
        help="tree-sitter grammar to be updated",
    )
    compute_metrics_cmd.set_defaults(func=compute_metrics)

    # Compare metrics command
    compare_metrics_cmd = commands.add_parser(
        "compare-metrics",
        parents=[common],
        help="Compares the metrics before and after "
        "a tree-sitter grammar update in order to discover whether "
        "there are differences.",
    )

    # Arguments
    compare_metrics_cmd.add_argument(
        "-g",
        "--grammar",
        type=str,
        required=True,
        help="tree-sitter grammar used to compare the metrics",
    )
    compare_metrics_cmd.set_defaults(func=compare_metrics)

    # Parse arguments
    args = parser.parse_args()

    # Call the command
    args.func(args)


if __name__ == "__main__":
    main()
