import os
import click
from click.utils import echo

import cli, render

def create_profile():
    create = render.yes_prompt('Create new radicle profile (key pair and initial configuration)')
    if not create:
        cli.run_exit()
    else:
        render.info("Creating new profile and radicle key...")
        cli.run_profile_create()

        render.info("Adding SSH-key...")
        profile = cli.run_profile_add_ssh()

        name = render.input_prompt('Please enter your username')
        person = cli.run_person_create(name)

        cli.run_person_set_default(person['urn'])

        render.success("Profile created successfully!", margin=False)
        render.profile(profile, person)

        render.info("To add a project to Radicle, run `rad init` in an existing git repository.")

def list_profiles():
    active_profile = cli.run_profile_get()
    profiles = cli.run_profile_list()

    if len(profiles) > 0:
        render.info("Found at least one existing profile.", margin=False)
        click.echo(f"- Profiles:")
        render.profile_list(profiles, active_profile, ['id'])

def create_project():
    render.info('Initializing new radicle project...')
    name = render.input_prompt('Please enter a name')
    branch = render.input_prompt('Default branch', default="main")
    
    render.info("Setting up `rad` remote...")
    project = cli.run_project_create(os.path.dirname(os.getcwd()), name, branch)
    
    if project:
        render.success("Project initialized successfully!", margin=False)
        render.project(project)
        
        render.info("To publish, run `rad publish` or `git push rad`")

def list_projects():
    # HACK: Compare with current folder name. Need to use more reliable check here.
    current_name = os.path.basename(os.getcwd())
    projects = cli.run_project_list()
    render.project_list(projects, current_name)

def publish_project():
    render.info('Publishing project...')
    cli.run_project_publish()

    render.info("To replicate to peers, run `rad node --setup`")

def setup_user_node(rad_home):
    render.info("Setting up local radicle node...")
    cli.run_user_node(rad_home)

def get_user_node():
    cli.run_user_node_info()
