# Changelog

## yamled 0.0.4

- An item whose last block scalar keeps its trailing lines (`|+`) can be taken, put, and reordered with those lines, and removing or inserting a key next to it no longer changes its value.

## yamled 0.0.3

- An item added after one whose block scalar keeps its trailing lines (`|+`) goes after those lines, so the earlier item keeps its value.

## yamled 0.0.2

- Reorder a block collection's children through fixed slots, re-indent a block collection, remove from and replace a one-line flow sequence in flow style, and insert a value at an index of a sequence.
- Fix eleven edit defects found by new property and YAML test suite tests: a replace keeps anchors, tags, and key-line comments; block scalars are replaced with their header and the trailing lines they own; empty values, explicit keys, and lists written at their key's column are handled or refused instead of corrupted; no edit panics.

## yamled 0.0.1

- Edit a YAML file in place: address a node by path or JSON pointer, then replace, remove, insert, push, take, or put values while every byte you did not name, comments included, stays as it was.

