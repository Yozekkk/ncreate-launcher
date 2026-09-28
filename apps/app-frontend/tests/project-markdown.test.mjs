import test from 'node:test'
import assert from 'node:assert/strict'
import { JSDOM } from 'jsdom'
import createDOMPurify from 'dompurify'
import { renderProjectMarkdown } from '../src/project-markdown.ts'

const dom = new JSDOM('')
const purify = createDOMPurify(dom.window)
function parsed(body) {
	const root = dom.window.document.createElement('div')
	root.innerHTML = renderProjectMarkdown(body, purify)
	return root
}

test('project prose renders headings, paragraphs, lists and code safely', () => {
	const root = parsed(
		'# Sodium\n\n**Fast** renderer.\n\n- Fabric\n- NeoForge\n\n```sh\njava -version\n```',
	)
	assert.equal(root.querySelector('h1').textContent, 'Sodium')
	assert.equal(root.querySelector('strong').textContent, 'Fast')
	assert.equal(root.querySelectorAll('li').length, 2)
	assert.equal(root.querySelector('pre code').textContent.trim(), 'java -version')
})
test('untrusted project content cannot execute scripts, navigate or load remote resources', () => {
	const root = parsed(
		'<script>alert(1)</script>\n\n<img src="https://evil.example/track" onerror="alert(1)">\n\n[Open](javascript:alert(1))\n\n[Public](https://example.com)\n\n![remote](https://evil.example/a.png)\n\n<iframe src="https://evil.example"></iframe>',
	)
	assert.equal(root.querySelector('script, img, iframe, a, style, input, form'), null)
	for (const element of root.querySelectorAll('*')) assert.equal(element.attributes.length, 0)
	assert.match(root.textContent, /Public/)
})
test('presentational HTML stays readable while fenced HTML examples remain escaped', () => {
	const root = parsed('<sup>Latest news</sup>\n\n```html\n<img onerror="bad()">\n```')
	assert.match(root.querySelector('p').textContent, /Latest news/)
	assert.doesNotMatch(root.querySelector('p').textContent, /sup/)
	assert.equal(root.querySelector('code').textContent.trim(), '<img onerror="bad()">')
})
