# shellcheck shell=bash

# Takes a string value and determines whether to consider it true or false.
# Returns 0 for true and 1 for false.
#
# Recommended usage for testing VAR, defaulting to false if unset:
#
#   if bool_true "${VAR:-}"; then
#       # deal with the truth
#   fi
#
bool_true() {
    if [ "$#" -ne 1 ]; then
        echo "bool_true(): unexpected arguments: \"$*\""
        exit 1
    fi

    # Empty string -> False
    if [ -z "$1" ]; then
        return 1; # False
    fi

    # Falsy word -> False
    case "$1" in
        0 | [nN][oO] | [fF][aA][lL][sS][eE])
            return 1; # False
    esac

    # Otherwise -> True
    return 0; # True
}
