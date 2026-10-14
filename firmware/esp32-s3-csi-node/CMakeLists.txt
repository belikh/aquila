# Aquila ESP32-S3 CSI node — ESP-IDF project.
#
# Build with ESP-IDF v5.x:
#   . "$IDF_PATH/export.sh"
#   idf.py set-target esp32s3
#   idf.py build
#   idf.py -p /dev/ttyUSB0 flash monitor
#
# Compile-verified with ESP-IDF v5.5.2 via `nix develop .#esp`; not flashed or
# run on hardware (see README.md).
cmake_minimum_required(VERSION 3.16)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(aquila_esp32s3_csi_node)
