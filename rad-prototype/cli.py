import subprocess
import json
import render
import click

def to_json(stdout):
    return json.loads(json.loads(stdout))

def run(commands):
    process = subprocess.run(commands,
        stdout=subprocess.PIPE,
        universal_newlines=True)
    return process.stdout

def run_deamon(commands):
    subprocess.Popen(commands)

def run_profile_create():
    stdout = run(['rad-profile-dev', 'create'])
    try:
        profile = to_json(stdout)
        return profile
    except:
        render.error("Could not create profile.", margin=False)

def run_profile_add_ssh():
    stdout = run(['rad-profile-dev', 'ssh', 'add'])
    try:
        profile = to_json(stdout)
        return profile
    except:
        render.error("Could not add SSH key.", margin=False)

def run_person_create(name):
    stdout = run(['rad-identities-dev', 'person', 'create', 'new', 
        '--payload', '{"name":"' + name +'"}'])
    try:
        person = json.loads(stdout)
        return person
    except:
        render.error("Could not create identity.", margin=False)

def run_person_set_default(urn):
    stdout = run(['rad-identities-dev', 'local', 'set', '--urn', urn])
    try:
        person = json.loads(stdout)
        return person
    except:
        render.error("Could not set default identity.", margin=False)

def run_profile_list():
    stdout = run(['rad-profile-dev', 'list'])
    try:
        profiles = to_json(stdout)
        return profiles
    except:
        render.error("Could not get list of profiles.", margin=False)
    
def run_profile_get():
    stdout = run(['rad-profile-dev', 'get'])
    try:
        profile = to_json(stdout)
        return profile
    except:
        render.warning("Could not find any profile.", margin=False)
        render.info("If you want to create a new profile, please use `rad auth`.")

def run_profile_paths():
    stdout = run(['rad-profile-dev', 'paths'])
    click.echo(f"{stdout}")

def run_project_list():
    stdout = run(['rad-identities-dev', 'project', 'list'])
    try:
        projects = json.loads(stdout)
        return projects
    except:
        render.error("Could not get list of projects.", margin=False)

def run_project_create(path, name, branch='master'):
    stdout = run(['rad-identities-dev', 'project', 'create', 'existing', 
        '--path', path,
        '--payload', '{"name":"' + name + '","default_branch":"' + branch + '"}'])
    
    try:
        project = json.loads(stdout)
        return project
    except:
        render.error("Could not get create project.", margin=False)

def run_project_publish():
    stdout = run(['git', 'push', 'rad'])
    click.echo()

def run_user_node(rad_home):
    stdout = run_deamon(['linkd',
        # '--protocol-listen', '0.0.0.0:8778'])
        '--rad-home', rad_home,
        '--protocol-listen', '0.0.0.0:8778'])
    click.echo(f"{stdout}")

def run_user_node_info():
    click.echo(f"Info...")

def run_exit():
    click.echo()
    exit()