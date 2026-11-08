#!/bin/zsh
# Regenerates every app icon from source: gen_icon.py -> icon-ios.svg -> PNG / .icns / AppIcon.appiconset.
# Needs Google Chrome (headless render), sips and iconutil (built into macOS).
# icon-ios-tinted.svg and icon-macos.svg are hand-maintained; icon-macos.svg embeds icon-ios.svg.
set -e
cd "${0:A:h}"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
render() { "$CHROME" --headless=new --disable-gpu --hide-scrollbars --default-background-color=00000000 \
  --force-device-scale-factor=1 --window-size=1024,1024 --allow-file-access-from-files \
  --screenshot="$PWD/$2" "file://$PWD/$1" >/dev/null 2>&1; }
python3 gen_icon.py icon-ios.svg
render icon-ios.svg icon-ios-1024.png
render icon-ios-tinted.svg icon-ios-tinted-1024.png
render icon-macos.svg icon-macos-1024.png
sips -s format png icon-ios-1024.png --out AppIcon.appiconset/AppIcon-1024.png >/dev/null
cp AppIcon.appiconset/AppIcon-1024.png AppIcon.appiconset/AppIcon-1024-dark.png
cp icon-ios-tinted-1024.png AppIcon.appiconset/AppIcon-1024-tinted.png
rm -rf AppIcon.iconset && mkdir AppIcon.iconset
for sz in 16 32 128 256 512; do
  sips -z $sz $sz icon-macos-1024.png --out AppIcon.iconset/icon_${sz}x${sz}.png >/dev/null
  sips -z $((sz*2)) $((sz*2)) icon-macos-1024.png --out AppIcon.iconset/icon_${sz}x${sz}@2x.png >/dev/null
done
iconutil -c icns AppIcon.iconset -o AppIcon.icns && rm -rf AppIcon.iconset
# The light pages' versions: the logo and the loader's sky and corona.
python3 gen_icon.py .light-light.svg all light && render .light-light.svg .light-light.png
sips -s format png -z 88 88 .light-light.png --out ../logo-light@2x.png >/dev/null
for layer in sky corona; do
  python3 gen_icon.py .light-$layer-light.svg $layer light
  render .light-$layer-light.svg .light-$layer-light.png
  sips -z 256 256 .light-$layer-light.png --out ../loader/$layer-light@2x.png >/dev/null
done
rm -f .light-*
echo "icons rebuilt"
