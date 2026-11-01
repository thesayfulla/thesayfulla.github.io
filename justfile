default:
    @just --list

# run the site locally
run port="8000":
    cargo run -- serve --port {{port}}

# export the public site into docs/
build-site:
    cargo run --release -- build-site

# make a user who can log in and write
createuser:
    cargo run -- create-user

# back up, build docs/, commit and push
publish:
    ./publish.sh
