import {
  Body,
  Controller,
  Headers,
  Post,
  UnauthorizedException,
} from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { ChainEventIngestDto } from './dto/chain-event-ingest.dto';
import { WebhooksService } from './webhooks.service';

@Controller('webhooks')
export class WebhooksController {
  constructor(
    private readonly webhooksService: WebhooksService,
    private readonly config: ConfigService,
  ) {}

  @Post('chain-events')
  async ingestChainEvent(
    @Headers('x-webhook-secret') secret: string | undefined,
    @Body() body: ChainEventIngestDto,
  ) {
    const expected = this.config.get<string>('WEBHOOK_INGEST_SECRET');
    if (!expected || secret !== expected) {
      throw new UnauthorizedException('Invalid webhook secret');
    }
    const row = await this.webhooksService.ingestChainEvent(body);
    return { id: row.id, dedupeKey: row.dedupeKey };
  }
}
