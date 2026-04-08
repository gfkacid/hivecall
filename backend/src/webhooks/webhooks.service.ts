import { Injectable } from '@nestjs/common';
import { PrismaService } from '../prisma/prisma.service';
import { ChainEventIngestDto } from './dto/chain-event-ingest.dto';

@Injectable()
export class WebhooksService {
  constructor(private readonly prisma: PrismaService) {}

  async ingestChainEvent(dto: ChainEventIngestDto) {
    return this.prisma.chainEvent.upsert({
      where: { dedupeKey: dto.dedupeKey },
      create: {
        chain: dto.chain,
        dedupeKey: dto.dedupeKey,
        source: dto.source,
        payload: dto.payload as object,
      },
      update: {
        payload: dto.payload as object,
        source: dto.source,
      },
    });
  }
}
