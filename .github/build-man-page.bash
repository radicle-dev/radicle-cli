#!/usr/bin/env bash

set -o errexit
set -o nounset
set -o pipefail

main () {
    if [[ $# -gt 1 ]]; then
        echo "Expected an .adoc file as the sole argument"
        return 1
    fi
    local adoc_file_path="$1"; shift
    asciidoctor --doctype manpage --backend manpage --out-file - "$adoc_file_path" | gzip --best --stdout
}

main "$@"
