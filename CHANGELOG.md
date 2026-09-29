# Changelog

## yamled 0.0.2

- Reorder a block collection's children through fixed slots, re-indent a block collection, remove from and replace a one-line flow sequence in flow style, and insert a value at an index of a sequence.
- Fix eleven edit defects found by new property and YAML test suite tests: a replace keeps anchors, tags, and key-line comments; block scalars are replaced with their header and the trailing lines they own; empty values, explicit keys, and lists written at their key's column are handled or refused instead of corrupted; no edit panics.

## yamled 0.0.1

- Edit a YAML file in place: address a node by path or JSON pointer, then replace, remove, insert, push, take, or put values while every byte you did not name, comments included, stays as it was.

