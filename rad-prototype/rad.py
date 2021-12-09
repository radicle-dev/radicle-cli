import time

import os
from os.path import exists
import subprocess

import json

import click
from click.decorators import password_option

import ui, render

RAD_HOME = os.getenv('RAD_HOME')

def valid_env():
    if RAD_HOME == None:
        click.echo("Could not read environment variable RAD_HOME.")
        return False
    return True

def profile_exists():
    if not exists(RAD_HOME + '/active_profile'):
        return False
    active_profile = open(RAD_HOME + '/active_profile', "r")
    id = active_profile.read()
    
    if not exists(RAD_HOME + '/' + id):
        click.echo(f"Could not find directory for profile {id}.")
        return False
    return True

def is_git_repo():
    if not exists('.git'):
        return False
    return True

def run_exit():
    exit()

# Main CLI
@click.group()
def main():
    # HACK: """Description needs to go here."""
    pass

@main.command()
@click.option("--init", is_flag=True)
@click.option("--list", is_flag=True)
def profile(init, list):
    if init:
        ui.create_profile()
    if list:
        ui.list_profiles()
    else:
        ui.list_profiles()

@main.command()
@click.option("--init", is_flag=True)
@click.option("--list", is_flag=True)
def project(init, list):
    if init:
        ui.create_project()
    if list:
        ui.list_projects()
    else:
        ui.list_projects()

# Main user flow
@main.command()
@click.option("--add", is_flag=True)
@click.option("--verbose", is_flag=True)
def auth(add, verbose):
    if not add:
        render.intro()
    if not add and profile_exists():
        ui.list_profiles()
        render.info("If you want to create a new profile, please use --add.")
        run_exit()
    else:
        ui.create_profile()

@main.command()
def init():
    if not is_git_repo():
        render.error("This is not a git repository.")
        run_exit()
    else:
        ui.create_project()

@main.command()
@click.option("--setup", is_flag=True)
def node(setup):
    if setup:
        ui.setup_user_node(RAD_HOME)
    else:
        ui.get_user_node()

@main.command()
def publish():
    if not is_git_repo():
        render.error("This is not a git repository.")
        run_exit()
    else:
        ui.publish_project()

if __name__ == "__main__":
    if valid_env():
        main()
    else:
        exit()
