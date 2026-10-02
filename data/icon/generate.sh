#!/bin/bash

magick viben64.png -resize 512x512 viben64_512x512.png
magick viben64.png -resize 256x256 viben64_256x256.png
magick viben64.png -resize 128x128 viben64_128x128.png
magick viben64.png -resize 256x256 icon.ico
