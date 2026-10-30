perl -0pi -e 's/\nroute = "\/tags\/\{term\}\/"\n/\nroute = "\/tags\/{term}\/"\npaginate = 5\n/' yuanme.toml
grep -q 'paginate = 5' yuanme.toml
