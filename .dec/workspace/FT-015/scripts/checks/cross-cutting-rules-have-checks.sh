#!/usr/bin/env bash
# TC-017: every_cross_cutting_adr_is_backed_by_a_runner_tc
# Enforces ADR-014 convention: every cross-cutting ADR should have a mechanical check
#
# Checks that every cross-cutting ADR with status=accepted has at least one TC
# with a non-empty runner field.
#
# Exit 0: all cross-cutting ADRs have runner TCs
# Exit 2 (warning): at least one cross-cutting ADR lacks a runner TC

set -euo pipefail

# Find .product/adrs directory
if [[ ! -d ".product/adrs" ]]; then
    # No .product directory yet - this is expected during bootstrap
    exit 0
fi

# Track ADRs without runner TCs
missing_runners=()

# Find all cross-cutting, accepted ADRs
for adr_file in .product/adrs/ADR-*.md; do
    [[ -e "$adr_file" ]] || continue
    
    # Extract frontmatter fields
    scope=$(awk '/^---$/,/^---$/ {if (/^scope:/) {print $2; exit}}' "$adr_file")
    status=$(awk '/^---$/,/^---$/ {if (/^status:/) {print $2; exit}}' "$adr_file")
    
    # Only check cross-cutting, accepted ADRs
    if [[ "$scope" != "cross-cutting" ]] || [[ "$status" != "accepted" ]]; then
        continue
    fi
    
    # Extract ADR id from filename (e.g., ADR-013 from ADR-013-code-quality.md)
    adr_id=$(basename "$adr_file" | grep -oE 'ADR-[0-9]+')
    
    # Search for TCs that validate this ADR and have a runner
    has_runner=false
    
    if [[ -d ".product/tests" ]]; then
        for tc_file in .product/tests/TC-*.md; do
            [[ -e "$tc_file" ]] || continue
            
            # Check if this TC validates our ADR
            validates_this_adr=false
            in_frontmatter=false
            in_validates_adrs=false
            
            while IFS= read -r line; do
                if [[ "$line" == "---" ]]; then
                    if [[ "$in_frontmatter" == "true" ]]; then
                        break
                    else
                        in_frontmatter=true
                        continue
                    fi
                fi
                
                if [[ "$in_frontmatter" == "true" ]]; then
                    # Check for validates.adrs section
                    if [[ "$line" =~ ^validates\.adrs: ]]; then
                        in_validates_adrs=true
                        # Check inline format: validates.adrs: [ADR-013, ADR-014]
                        if echo "$line" | grep -q "$adr_id"; then
                            validates_this_adr=true
                        fi
                    elif [[ "$in_validates_adrs" == "true" ]]; then
                        # Check list format
                        if [[ "$line" =~ ^[[:space:]]*- ]]; then
                            if echo "$line" | grep -q "$adr_id"; then
                                validates_this_adr=true
                            fi
                        else
                            # End of validates.adrs section
                            in_validates_adrs=false
                        fi
                    fi
                    
                    # Check for non-empty runner
                    if [[ "$validates_this_adr" == "true" ]]; then
                        if [[ "$line" =~ ^runner: ]]; then
                            runner_value=$(echo "$line" | sed 's/^runner:[[:space:]]*//')
                            if [[ -n "$runner_value" && "$runner_value" != "null" ]]; then
                                has_runner=true
                                break
                            fi
                        fi
                    fi
                fi
            done < "$tc_file"
            
            if [[ "$has_runner" == "true" ]]; then
                break
            fi
        done
    fi
    
    if [[ "$has_runner" == "false" ]]; then
        missing_runners+=("$adr_id")
    fi
done

# Report results
if [[ ${#missing_runners[@]} -gt 0 ]]; then
    echo "WARNING: The following cross-cutting ADRs lack a runner TC:"
    for adr_id in "${missing_runners[@]}"; do
        echo "  $adr_id"
    done
    exit 2
fi

exit 0
