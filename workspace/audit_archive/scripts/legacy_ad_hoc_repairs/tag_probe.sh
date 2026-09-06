#!/usr/bin/env bash
# ==============================================================================
# scripts/tag_probe.sh — Symfonium Tag Probe Script (S177)
# Idempotent tagging probe across 6 representative FLAC tracks
# ==============================================================================
set -eu

PROBE_DIR="/tmp/tag_audit"
PROBE_LIST="${PROBE_DIR}/probe_files.txt"

mkdir -p "${PROBE_DIR}"

# Select 6 FLACs if not already selected or if list is empty
if [ ! -f "${PROBE_LIST}" ] || [ ! -s "${PROBE_LIST}" ]; then
  echo "==> Seleccionando 6 archivos FLAC representativos..."
  find ~/Music/Syncify -type f -name '*.flac' | shuf -n 6 > "${PROBE_LIST}"
fi

echo "======================================================================"
echo " SYMFONIUM EXTENDED TAG PROBE (6 ARCHIVOS)"
echo "======================================================================"

file_idx=0
while IFS= read -r f; do
  [ -z "$f" ] && continue
  if [ ! -f "$f" ]; then
    echo "WARN: Archivo no encontrado: $f"
    continue
  fi

  file_idx=$((file_idx + 1))
  echo ""
  echo "----------------------------------------------------------------------"
  echo "[$file_idx/6] Procesando: $(basename "$f")"
  echo "      Ruta: $f"
  echo "----------------------------------------------------------------------"

  # 1. Remover tags previos de sondeo (idempotencia)
  metaflac \
    --remove-tag=LANGUAGE \
    --remove-tag=STYLE \
    --remove-tag=ALBUMSTYLE \
    --remove-tag=TRACKSTYLE \
    --remove-tag=MOOD \
    --remove-tag=ALBUMMOOD \
    --remove-tag=TRACKMOOD \
    --remove-tag=TAGS \
    --remove-tag=ALBUMTAGS \
    --remove-tag=ARTISTTAGS \
    --remove-tag=COMPILATION \
    --remove-tag=GROUPING \
    --remove-tag=OCCASION \
    --remove-tag=MEDIA \
    --remove-tag=MUSICTYPE \
    "$f"

  # 2. Aplicar según variante
  if [ "$file_idx" -eq 5 ]; then
    echo "  -> Aplicando Variante: Separador Slash (/)"
    metaflac \
      --set-tag=LANGUAGE=English \
      --set-tag=LANGUAGE=en \
      --set-tag=LANGUAGE=eng \
      --set-tag="STYLE=Shoegaze/Dream Pop" \
      --set-tag="ALBUMSTYLE=Gothic Rock" \
      --set-tag="TRACKSTYLE=Post-Punk" \
      --set-tag="MOOD=Energetic/Melancholy" \
      --set-tag="ALBUMMOOD=Dark" \
      --set-tag="TRACKMOOD=Upbeat" \
      --set-tag="TAGS=80s/Synthpop/New Wave" \
      --set-tag="ALBUMTAGS=Remaster" \
      --set-tag="ARTISTTAGS=British" \
      --set-tag="COMPILATION=0" \
      --set-tag="GROUPING=Duran Duran - Studio Albums" \
      --set-tag="OCCASION=Party" \
      --set-tag="MEDIA=SOUNDTRACK" \
      --set-tag="MUSICTYPE=Soundtrack" \
      "$f"
  elif [ "$file_idx" -eq 6 ]; then
    echo "  -> Aplicando Variante: Multi-valor Vorbis nativo (múltiples entradas)"
    metaflac \
      --set-tag=LANGUAGE=English \
      --set-tag=LANGUAGE=en \
      --set-tag=LANGUAGE=eng \
      --set-tag="STYLE=Shoegaze" \
      --set-tag="STYLE=Dream Pop" \
      --set-tag="ALBUMSTYLE=Gothic Rock" \
      --set-tag="TRACKSTYLE=Post-Punk" \
      --set-tag="MOOD=Energetic" \
      --set-tag="MOOD=Melancholy" \
      --set-tag="ALBUMMOOD=Dark" \
      --set-tag="TRACKMOOD=Upbeat" \
      --set-tag="TAGS=80s" \
      --set-tag="TAGS=Synthpop" \
      --set-tag="TAGS=New Wave" \
      --set-tag="ALBUMTAGS=Remaster" \
      --set-tag="ARTISTTAGS=British" \
      --set-tag="COMPILATION=0" \
      --set-tag="GROUPING=Duran Duran - Studio Albums" \
      --set-tag="OCCASION=Party" \
      --set-tag="MEDIA=SOUNDTRACK" \
      --set-tag="MUSICTYPE=Soundtrack" \
      "$f"
  else
    echo "  -> Aplicando Variante: Separador Punto y Coma (;)"
    metaflac \
      --set-tag=LANGUAGE=English \
      --set-tag=LANGUAGE=en \
      --set-tag=LANGUAGE=eng \
      --set-tag="STYLE=Shoegaze; Dream Pop" \
      --set-tag="ALBUMSTYLE=Gothic Rock" \
      --set-tag="TRACKSTYLE=Post-Punk" \
      --set-tag="MOOD=Energetic; Melancholy" \
      --set-tag="ALBUMMOOD=Dark" \
      --set-tag="TRACKMOOD=Upbeat" \
      --set-tag="TAGS=80s; Synthpop; New Wave" \
      --set-tag="ALBUMTAGS=Remaster" \
      --set-tag="ARTISTTAGS=British" \
      --set-tag="COMPILATION=0" \
      --set-tag="GROUPING=Duran Duran - Studio Albums" \
      --set-tag="OCCASION=Party" \
      --set-tag="MEDIA=SOUNDTRACK" \
      --set-tag="MUSICTYPE=Soundtrack" \
      "$f"
  fi

  # 3. Verificación post-escritura
  after_tags=$(metaflac --export-tags-to=- "$f")
  
  for required_tag in STYLE MOOD TAGS COMPILATION GROUPING OCCASION MEDIA MUSICTYPE; do
    if ! echo "$after_tags" | grep -qi "^${required_tag}="; then
      echo "ERROR: Tag requerido '${required_tag}' no encontrado tras escritura en $f" >&2
      exit 1
    fi
  done

  echo "  ✓ Verificación post-escritura exitosa para $(basename "$f")"
  echo "  Tags aplicados destacados:"
  echo "$after_tags" | grep -E "^(LANGUAGE|STYLE|ALBUMSTYLE|TRACKSTYLE|MOOD|ALBUMMOOD|TRACKMOOD|TAGS|ALBUMTAGS|ARTISTTAGS|COMPILATION|GROUPING|OCCASION|MEDIA|MUSICTYPE)=" | sed 's/^/    • /'

done < "${PROBE_LIST}"

echo ""
echo "======================================================================"
echo " ¡SONDEO COMPLETADO EXITOSAMENTE EN LOS 6 ARCHIVOS!"
echo " Lista de archivos guardada en: ${PROBE_LIST}"
echo "======================================================================"
