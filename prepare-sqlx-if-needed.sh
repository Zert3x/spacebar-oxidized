#!/bin/bash

if git diff --quiet HEAD -- "./migrations"; then
    echo "No changes in migrations folder"
    exit 0
else
    echo "Changes in migrations folder detected, running sqlx prepare"
    cargo sqlx prepare -- --all-features --all-targets
    git add .sqlx/*
    exit_code=$?

    exit $exit_code
fi
