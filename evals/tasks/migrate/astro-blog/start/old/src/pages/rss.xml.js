import rss from '@astrojs/rss';
import { getCollection } from 'astro:content';

export async function GET(context) {
	const posts = await getCollection('blog');
	return rss({
		title: 'Notes by Sam',
		description: 'Compilers, small tools, and the occasional proof.',
		site: context.site,
		items: posts.map((post) => ({ ...post.data, link: `/blog/${post.id}/` })),
	});
}
