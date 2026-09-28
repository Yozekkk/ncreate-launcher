import MarkdownIt from 'markdown-it'
import DOMPurify from 'dompurify'

const markdown = new MarkdownIt({ html: false, linkify: false, breaks: true })
// Project prose can contain presentational HTML; keep its text without exposing tags.
markdown.renderer.rules.text = (tokens, index) =>
	markdown.utils.escapeHtml(
		tokens[index].content.replace(
			/<\/?(?:sup|sub|details|summary|div|span|center|p|a|img)\b[^>]*>/gi,
			'',
		),
	)

export function renderProjectMarkdown(
	body: string,
	purify: Pick<typeof DOMPurify, 'sanitize'> = DOMPurify,
): string {
	return purify.sanitize(markdown.render(body.slice(0, 500_000)), {
		ALLOWED_TAGS: [
			'p',
			'h1',
			'h2',
			'h3',
			'h4',
			'h5',
			'h6',
			'ul',
			'ol',
			'li',
			'strong',
			'em',
			's',
			'blockquote',
			'pre',
			'code',
			'br',
			'hr',
			'table',
			'thead',
			'tbody',
			'tr',
			'th',
			'td',
		],
		ALLOWED_ATTR: [],
		ALLOW_DATA_ATTR: false,
		ALLOW_ARIA_ATTR: false,
	})
}
