mv images/hero.png static/hero.png
perl -pi -e 's|^\{\{ image\("images/hero.png".*$|<img class="hero" src="/hero.png" alt="Hills at sunrise" width="1600" height="340" loading="lazy">|' templates/home.html
