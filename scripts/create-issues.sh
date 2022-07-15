#!/bin/sh

rad issue new --title "Update Cypress to v10" --description "As of this July, Cypress has been released in its tenth version."
rad issue new --title "Document reused components" --description "Where needed we should add some documentation on the how and why of some shared components."
rad issue new --title "Improve the switching between states" --description "At the moment when switching branches, or views in the project component we don't have a desktop experience."
rad sync

