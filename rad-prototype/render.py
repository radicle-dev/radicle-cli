import click
from pyfiglet import Figlet

ident_url = "https://radicle.xyz/link/identities/person/v1"
project_url = "https://radicle.xyz/link/identities/project/v1"

def logo():
    f = Figlet(font='slant')
    print(f.renderText('text to render'))

def intro():
    click.echo(click.style("🌱 Welcome to Radicle CLI!", fg="magenta", bold=True))
    click.echo()

def error(error, margin=False):
    click.echo(click.style("x", fg="red") + f" {error}")
    if margin:
        click.echo()


def warning(warning, margin=False):
    click.echo(click.style("!", fg="bright_yellow") + f" {warning}")
    if margin:
        click.echo()

def info(info, margin=False):
    click.echo(click.style("i", fg="blue") + f" {info}")
    if margin:
        click.echo()

def success(info, margin=False):
    click.echo(click.style("✓", fg="green") + f" {info}")
    if margin:
        click.echo()

def project_list(projects, active_project):
    click.echo(f"- Projects:")
    for project in projects:
        name = project['payload'][project_url]['name']
        if name == active_project:
            click.echo(click.style(" ⊙ ", fg="bright_yellow") + f"{name}" + click.style(" (current)", fg="bright_yellow"))
        else:
            click.echo(f" ⋅ {name}")

def project(project, margin=False):
    click.echo(f"- Project:")
    click.echo(f"  ⋅ Name ->" + f" {project['payload'][project_url]['name']}")
    click.echo(f"  ⋅ URN  ->" + f" {project['urn']}")
    if margin:
        click.echo()
            
def profile_list(items, active_item, fields):
    for item in items:
        if item[fields[0]] == active_item[fields[0]]:
            click.echo(click.style(" ⊙ ", fg="bright_yellow") + f"{item[fields[0]]}" + click.style(" (active)", fg="bright_yellow"))
        else:
            click.echo(f" ⋅ {item[fields[0]]}")

def profile(profile, person, margin=False):
    click.echo(f"- Profile:")
    click.echo(f"  ⋅ Name ->" + f" {person['payload'][ident_url]['name']}")
    click.echo(f"  ⋅ Id   ->" + f" {profile['id']}")
    click.echo(f"  ⋅ URN  ->" + f" {person['urn']}")
    if margin:
        click.echo()

def yes_prompt(prompt):
    create = click.prompt(click.style("?", fg="green") + " " + prompt, 
        type=click.Choice(['Y', 'n']), show_choices=True)
    return create == 'Y'

def input_prompt(prompt, default=None):
    if default != None:
        value = click.prompt(click.style("?", fg="green") + " " + prompt, default=default)
    else:
        value = click.prompt(click.style("?", fg="green") + " " + prompt)
    return value