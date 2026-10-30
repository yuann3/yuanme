perl -pi -e 's/^title = "My Site"/title = "Grace Hopper"/; s/^author = "Your Name"/author = "Grace Hopper"/' yuanme.toml
perl -pi -e 's|\{"href": "/about/", "name": "About"\}|{"href": "/resume/", "name": "Résumé"}, {"href": "/about/", "name": "About"}|' templates/layouts/base.html
