mkdir -p images
mv static/hero.png images/hero.png
perl -pi -e 's|^<img class="hero" src="/hero.png" alt="Hills at sunrise" width="1600" height="340">$|{{ image("images/hero.png", sizes="(min-width: 44rem) 40rem, 100vw") \| picture(alt="Hills at sunrise", class="hero", priority=true) }}|' templates/home.html
grep -q 'image("images/hero.png"' templates/home.html
