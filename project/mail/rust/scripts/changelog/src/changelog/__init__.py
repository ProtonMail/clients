from functools import partial
import re
from collections import OrderedDict
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

import click
from git import Commit, Repo, Tag

from changelog.template import render
from changelog.types import Commits, Tags

ClickPath = partial(click.Path, path_type=Path)


@click.command()
@click.option("--repo", type=ClickPath(), help="The git repository to analyze")
@click.option("--only", type=str, help="Only include matching tags")
@click.option("--head", type=str, help="The current commit")
@click.option("--init", type=str, help="The initial commit")
@click.option("--name", type=str, help="The release name")
@click.option("--path", type=ClickPath(), multiple=True, help="The path(s) to analyze")
def main(
    repo: Path | None,
    only: str | None,
    head: str | None,
    init: str | None,
    name: str | None,
    path: tuple[Path, ...],
) -> None:
    with open_repo(repo) as r:
        only_re = re.compile(only) if only else None
        tags = {t.commit: t for t in r.tags if not only_re or only_re.match(t.name)}
        commits, cur_tag = OrderedDict(), None

        if path:
            path_commits = set(
                r.iter_commits(f"{init or ''}..{r.commit(head)}", paths=path)
            )
        else:
            path_commits = None

        for c in r.iter_commits(f"{init or ''}..{r.commit(head)}"):
            cur_tag = tags.get(c) or cur_tag
            if path_commits is None or c in path_commits:
                commits.setdefault(cur_tag, []).append(c)

        print(render(commits, only_re, name))


@contextmanager
def open_repo(repo: Path | None) -> Iterator[Repo]:
    try:
        yield (r := Repo(repo))
    finally:
        r.close()
