#!/usr/bin/env python
import os.path as osp
import requests
from urllib.parse import urljoin
import json
import hashlib
import click

MAX_PKG_SIZE = 100 * 1024 * 1024

API_TPLS = {
    'repo.prefix.dev': {
        'upload': "https://prefix.dev/api/v1/upload/{channel}",
        'delete': "https://prefix.dev//api/v1/delete/{channel}/{subdir}/{pkg}",
    }
}

@click.group()
@click.option('-k', '--token', default='~/.mamba/auth/authentication.json')
@click.option('-r', '--repo', default='repo.prefix.dev')
@click.option('-c', '--channel', default='vidlg')
@click.pass_context
def cli(ctx, token, repo, channel):
    if token.endswith('.json'):
        o = json.load(open(osp.expanduser(token)))
        token = o[repo]['token']

    ctx.obj.update({
        'token': token,
        'repo': repo,
        'channel': channel,
    })

@cli.command()
@click.argument("pkgs", nargs=-1)
@click.pass_context
def upload(ctx, pkgs):
    channel = ctx.obj['channel']
    repo = ctx.obj['repo']
    token = ctx.obj['token']
    upload_url = API_TPLS[repo]['upload'].format(channel=channel)

    for pkg in pkgs:
        data = open(pkg, 'rb').read()

        # skip if larger than 100Mb
        if len(data) > MAX_PKG_SIZE:
            print(f"Skipping {pkg} because it is too large")
            return

        name = osp.basename(pkg)
        sha256 = hashlib.sha256(data).hexdigest()
        headers = {
            "X-File-Name": name,
            "X-File-SHA256": sha256,
            "Authorization": f"Bearer {token}",
            "Content-Length": str(len(data)),
            "Content-Type": "application/octet-stream",
        }

        print (f"uploading package {name} to {upload_url}")
        r = requests.post(upload_url, data=data, headers=headers)
        print(f"uploaded package {name} with status  {r.status_code}")

@cli.command()
@click.argument("pkgs", nargs=-1)
@click.pass_context
def delete(ctx, pkgs):
    channel = ctx.obj['channel']
    repo = ctx.obj['repo']
    token = ctx.obj['token']
    for pkg in pkgs:
        subdir = osp.basename(osp.dirname(pkg))
        pkg_name = osp.basename(pkg)
        delete_url = API_TPLS[repo]['delete'].format(channel=channel, subdir=subdir, pkg=pkg_name)
        headers = {
            "Authorization": f"Bearer {token}",
        }

        print (f"deleting package {pkg_name} with {delete_url}")
        r = requests.delete(delete_url, headers=headers)
        print(f"deleted package {pkg_name} with status  {r.status_code}")

if __name__ == "__main__":
    cli(obj={})