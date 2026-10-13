# Aquila ESP32-S3 CSI node — ESP-IDF project.
#
# Build with ESP-IDF v5.x:
#   . "$IDF_PATH/export.sh"
#   idf.py set-target esp32s3
#   idf.py build
#   idf.py -p /dev/ttyUSB0 flash monitor
#
# This project is a reference implementation; it is not built, flashed or tested
# in the Aquila repository (see README.md).
cmake_minimum_required(VERSION 3.16)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(aquila_esp32s3_csi_node)
