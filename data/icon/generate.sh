#!/bin/bash

magick vibe64.png -resize 512x512 vibe64_512x512.png
magick vibe64.png -resize 256x256 vibe64_256x256.png
magick vibe64.png -resize 128x128 vibe64_128x128.png
magick vibe64.png -resize 256x256 icon.ico
