#!/bin/bash
# phpr-target-bundle.sh mount|umount|status|compact — apparato S-182 (decisione
# utente 2026-09-29): sparsebundle APFS sul volume esterno che ospita le target
# INCREMENTALI (dev-output, ci-target). La canonica ~/Claude/php-rust-output NON
# c'entra: resta sul disco interno con la ricetta del pin.
# Il mountpoint è bloccato (chflags uchg): da smontata NON è scrivibile, quindi
# cargo fallisce rumorosamente invece di riempire il disco interno.
set -euo pipefail
B="/Volumes/Extreme Pro/Claude/phpr-target.sparsebundle"
MP="$HOME/Claude/phpr-target"
case "${1:-status}" in
  mount)   mount | grep -q " $MP " && { echo "già montata: $MP"; exit 0; }
           hdiutil attach -nobrowse -mountpoint "$MP" "$B" -quiet && echo "montata: $MP" ;;
  umount)  hdiutil detach "$MP" -quiet && echo "smontata: $MP" ;;
  compact) hdiutil detach "$MP" -quiet; hdiutil compact "$B" | tail -1
           hdiutil attach -nobrowse -mountpoint "$MP" "$B" -quiet; echo "compattata e rimontata" ;;
  status)  if mount | grep -q " $MP "; then echo "MONTATA $MP"; df -h "$MP" | tail -1 | awk '{print "  bundle: size="$2" used="$3" free="$4}'
           else echo "NON MONTATA ($MP)"; fi
           du -sh "$B" 2>/dev/null | awk '{print "  su disco esterno: "$1}' ;;
  *) echo "uso: $0 mount|umount|status|compact"; exit 2 ;;
esac
