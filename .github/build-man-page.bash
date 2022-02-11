#!/usr/bin/env bash

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace
export SHELLOPTS

main () {
    if [[ $# -eq 0 ]]; then
        echo "Usage: $0 FILE.adoc..."
        return 1
    fi

    while [[ $# -gt 0 ]]; do
        local adoc_file_path="$1"; shift
        local output_file_path="$(basename "$adoc_file_path" .adoc).gz"
        asciidoctor --doctype manpage --backend manpage --out-file - "$adoc_file_path" | gzip --best --stdout >"$output_file_path"
    done
}

main "$@"
